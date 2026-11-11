[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[A-Za-z0-9.\-]{3,50}$')]
    [string]$IdentityName,
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^CN=.+$')]
    [string]$Publisher,
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$PublisherDisplayName,
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^\d+\.\d+\.\d+\.\d+$')]
    [string]$Version,
    [ValidateSet('debug', 'release')]
    [string]$Configuration = 'release',
    [string]$OutputDirectory = 'artifacts\msix',
    [string]$CertificatePath,
    [SecureString]$CertificatePassword
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$manifestTemplate = Join-Path $PSScriptRoot 'AppxManifest.xml.template'
$assetsRoot = Join-Path $PSScriptRoot 'Assets'
$outputRoot = Join-Path $repoRoot $OutputDirectory
$stageRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("otto-os-msix-" + [guid]::NewGuid().ToString('N'))
$configurationArgument = if ($Configuration -eq 'release') { '--release' } else { '' }
$targetRoot = Join-Path $repoRoot ("target\$Configuration")
$executable = Join-Path $targetRoot 'otto-os.exe'
$makeAppx = Get-Command makeappx.exe -ErrorAction SilentlyContinue
$signTool = Get-Command signtool.exe -ErrorAction SilentlyContinue

function Get-DiscoveredToolPath {
    param([object]$Tool)

    if ($Tool -and $Tool.PSObject.Properties['Path'] -and $Tool.Path) {
        return $Tool.Path
    }
    if ($Tool -and $Tool.PSObject.Properties['FullName'] -and $Tool.FullName) {
        return $Tool.FullName
    }
    return $null
}

if (-not $makeAppx) {
    $makeAppx = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Filter makeappx.exe -Recurse -ErrorAction SilentlyContinue |
        Sort-Object FullName -Descending |
        Select-Object -First 1
}
if (-not $makeAppx) {
    throw 'makeappx.exe was not found. Install the Windows 10/11 SDK.'
}
$makeAppxPath = Get-DiscoveredToolPath $makeAppx
if (-not $makeAppxPath) {
    throw 'makeappx.exe path could not be resolved.'
}

foreach ($asset in @('StoreLogo.png', 'Square150x150Logo.png', 'Square44x44Logo.png')) {
    $assetPath = Join-Path $assetsRoot $asset
    if (-not (Test-Path -LiteralPath $assetPath -PathType Leaf)) {
        throw "Missing required package asset: $assetPath"
    }
}

$cargoArguments = @('build', '-p', 'otto-os')
if ($configurationArgument) {
    $cargoArguments += $configurationArgument
}
Push-Location $repoRoot
try {
    & cargo @cargoArguments
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
    throw "Build output was not found: $executable"
}

New-Item -ItemType Directory -Path $stageRoot -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $stageRoot 'Assets') -Force | Out-Null
New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
try {
    $manifest = Get-Content -LiteralPath $manifestTemplate -Raw
    $escapeXml = {
        param([string]$value)
        [System.Security.SecurityElement]::Escape($value)
    }
    $manifest = $manifest.Replace('__IDENTITY_NAME__', (& $escapeXml $IdentityName))
    $manifest = $manifest.Replace('__PUBLISHER__', (& $escapeXml $Publisher))
    $manifest = $manifest.Replace('__PUBLISHER_DISPLAY_NAME__', (& $escapeXml $PublisherDisplayName))
    $manifest = $manifest.Replace('__VERSION__', (& $escapeXml $Version))
    Set-Content -LiteralPath (Join-Path $stageRoot 'AppxManifest.xml') -Value $manifest -Encoding utf8NoBOM
    Copy-Item -LiteralPath $executable -Destination (Join-Path $stageRoot 'otto-os.exe')
    Copy-Item -LiteralPath (Join-Path $assetsRoot 'StoreLogo.png') -Destination (Join-Path $stageRoot 'Assets\StoreLogo.png')
    Copy-Item -LiteralPath (Join-Path $assetsRoot 'Square150x150Logo.png') -Destination (Join-Path $stageRoot 'Assets\Square150x150Logo.png')
    Copy-Item -LiteralPath (Join-Path $assetsRoot 'Square44x44Logo.png') -Destination (Join-Path $stageRoot 'Assets\Square44x44Logo.png')

    $packagePath = Join-Path $outputRoot ("OttoOS_$Version.msix")
    & $makeAppxPath pack /d $stageRoot /p $packagePath /o
    if ($LASTEXITCODE -ne 0) {
        throw "makeappx.exe failed with exit code $LASTEXITCODE"
    }

    if ($CertificatePath) {
        if (-not $signTool) {
            $signTool = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Filter signtool.exe -Recurse -ErrorAction SilentlyContinue |
                Sort-Object FullName -Descending |
                Select-Object -First 1
        }
        if (-not $signTool) {
            throw 'signtool.exe was not found. Install the Windows 10/11 SDK.'
        }
        $signToolPath = Get-DiscoveredToolPath $signTool
        if (-not $signToolPath) {
            throw 'signtool.exe path could not be resolved.'
        }
        if (-not (Test-Path -LiteralPath $CertificatePath -PathType Leaf)) {
            throw "Certificate was not found: $CertificatePath"
        }
        $signArguments = @('sign', '/fd', 'SHA256', '/a', '/f', $CertificatePath)
        if ($CertificatePassword) {
            $signArguments += @('/p', ([System.Net.NetworkCredential]::new('', $CertificatePassword).Password))
        }
        $signArguments += $packagePath
        & $signToolPath @signArguments
        if ($LASTEXITCODE -ne 0) {
            throw "signtool.exe failed with exit code $LASTEXITCODE"
        }
        Write-Output "SIGNED $packagePath"
    }
    else {
        Write-Output "UNSIGNED $packagePath"
        Write-Output 'A Store-compatible publisher certificate is required before submission.'
    }
}
finally {
    if (Test-Path -LiteralPath $stageRoot) {
        Remove-Item -LiteralPath $stageRoot -Recurse -Force
    }
}
