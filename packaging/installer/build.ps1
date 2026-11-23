[CmdletBinding()]
param(
    [string]$Version = '',
    [string]$ExecutablePath = '',
    [string]$OutputDir = 'artifacts\installer',
    [string]$CertificatePath = '',
    [Security.SecureString]$CertificatePassword,
    [switch]$SkipBuild
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Resolve-CargoVersion {
    param([string]$RepoRoot)

    $cargoToml = Get-Content -LiteralPath (Join-Path $RepoRoot 'Cargo.toml') -Raw
    if ($cargoToml -match '(?m)^version\s*=\s*"([^"]+)"') {
        return $Matches[1]
    }
    throw 'Could not parse workspace version from Cargo.toml'
}

function Find-InnoCompiler {
    $cmd = Get-Command 'ISCC.exe' -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty Source
    if ($cmd) {
        return $cmd
    }

    $candidates = @(
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe')
    ) | Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Leaf) }

    $found = $candidates | Select-Object -First 1
    if (-not $found) {
        throw 'ISCC.exe (Inno Setup 6) was not found. Install Inno Setup 6 and retry.'
    }
    return $found
}

function Find-SignTool {
    $cmd = Get-Command 'signtool.exe' -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty Source
    if ($cmd) {
        return $cmd
    }

    $kitsBin = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    if (Test-Path -LiteralPath $kitsBin -PathType Container) {
        $candidate = Get-ChildItem -Path $kitsBin -Filter 'signtool.exe' -Recurse -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -like '*\x64\*' } |
            Sort-Object FullName -Descending |
            Select-Object -First 1
        if ($candidate) {
            return $candidate.FullName
        }
    }
    throw 'signtool.exe was not found in PATH or Windows Kits 10 bin.'
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Push-Location $repoRoot
try {
    if (-not $Version) {
        $Version = Resolve-CargoVersion -RepoRoot $repoRoot
    }
    $numericVersion = $Version -replace '-.*$', ''

    if (-not $ExecutablePath) {
        $ExecutablePath = Join-Path $repoRoot 'target\release\otto-os.exe'
    }
    if (-not $SkipBuild -and -not (Test-Path -LiteralPath $ExecutablePath -PathType Leaf)) {
        cargo build --release -p otto-os
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build --release -p otto-os failed with exit code $LASTEXITCODE"
        }
    }
    $resolvedExe = (Resolve-Path -LiteralPath $ExecutablePath).Path

    $resolvedOut = if ([System.IO.Path]::IsPathRooted($OutputDir)) {
        $OutputDir
    } else {
        Join-Path $repoRoot $OutputDir
    }
    $null = New-Item -ItemType Directory -Path $resolvedOut -Force

    $iscc = Find-InnoCompiler
    $issPath = Join-Path $PSScriptRoot 'otto-os.iss'
    & $iscc "/DMyAppVersion=$numericVersion" "/DSourceExe=$resolvedExe" "/DRepoRoot=$repoRoot" "/O$resolvedOut" $issPath
    if ($LASTEXITCODE -ne 0) {
        throw "ISCC.exe failed with exit code $LASTEXITCODE"
    }

    $installerPath = Join-Path $resolvedOut "otto-os-setup-$numericVersion-x64.exe"
    if (-not (Test-Path -LiteralPath $installerPath -PathType Leaf)) {
        throw "Expected installer was not produced: $installerPath"
    }

    if ($CertificatePath) {
        $signTool = Find-SignTool
        $signArgs = @('sign', '/fd', 'SHA256', '/tr', 'http://timestamp.digicert.com', '/td', 'SHA256', '/f', $CertificatePath)
        if ($CertificatePassword) {
            $signArgs += @('/p', ([System.Net.NetworkCredential]::new('', $CertificatePassword).Password))
        }
        $signArgs += $installerPath
        & $signTool @signArgs
        if ($LASTEXITCODE -ne 0) {
            throw "signtool.exe failed with exit code $LASTEXITCODE"
        }
    }

    $hash = (Get-FileHash -LiteralPath $installerPath -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-Output "INSTALLER $installerPath"
    Write-Output "SHA256 $hash"
}
finally {
    Pop-Location
}
