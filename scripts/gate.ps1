param(
    [switch]$SkipLanding,
    [switch]$RequireCargoDeny
)

$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)

$steps = @(
    @{ Name = 'fmt';            Command = 'cargo';      Args = @('fmt', '--all', '--', '--check') },
    @{ Name = 'check';          Command = 'cargo';      Args = @('check', '--workspace') },
    @{ Name = 'clippy';         Command = 'cargo';      Args = @('clippy', '--workspace', '--all-targets', '--all-features', '--', '-D', 'warnings') },
    @{ Name = 'test-core-app';  Command = 'cargo';      Args = @('test', '-p', 'otto-core', '-p', 'otto-render', '-p', 'otto-os', '--', '--test-threads=1') },
    @{ Name = 'test-platform';  Command = 'cargo';      Args = @('test', '-p', 'otto-platform', '--', '--skip', 'the_screen_grabber_returns_a_frame_of_the_requested_size', '--test-threads=1') },
    @{ Name = 'e2e-static';     Command = 'powershell'; Args = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', 'scripts/check-e2e.ps1') }
)

$failed = @()
foreach ($step in $steps) {
    Write-Output "== $($step.Name)"
    & $step.Command @($step.Args)
    if ($LASTEXITCODE -ne 0) {
        $failed += $step.Name
    }
}

Write-Output '== cargo-deny'
if (Get-Command cargo-deny -ErrorAction SilentlyContinue) {
    cargo deny check --all-features
    if ($LASTEXITCODE -ne 0) {
        $failed += 'cargo-deny'
    }
} elseif ($RequireCargoDeny) {
    Write-Output ' - cargo-deny not found on PATH (required by -RequireCargoDeny)'
    $failed += 'cargo-deny'
} else {
    Write-Output ' - SKIPPED: cargo-deny not installed locally (enforced in CI; install via cargo install cargo-deny --locked)'
}

Write-Output '== ps1-syntax'
$psErrors = @()
foreach ($dir in 'scripts', 'packaging', 'tests/e2e') {
    if (-not (Test-Path -LiteralPath $dir)) { continue }
    foreach ($file in (Get-ChildItem -Path $dir -Filter '*.ps1' -Recurse -File)) {
        $tokens = $null
        $errors = $null
        [void][System.Management.Automation.Language.Parser]::ParseFile($file.FullName, [ref]$tokens, [ref]$errors)
        foreach ($err in $errors) {
            $psErrors += "$($file.FullName): $($err.Message)"
        }
    }
}
if ($psErrors.Count -gt 0) {
    foreach ($msg in $psErrors) { Write-Output " - $msg" }
    $failed += 'ps1-syntax'
}

if (-not $SkipLanding) {
    if (Get-Command bun -ErrorAction SilentlyContinue) {
        Push-Location 'landing'
        try {
            $landingSteps = @(
                @{ Name = 'landing-biome'; Command = 'bun'; Args = @('x', 'biome', 'ci', '.') },
                @{ Name = 'landing-astro'; Command = 'bun'; Args = @('run', 'check') },
                @{ Name = 'landing-tsc';   Command = 'bun'; Args = @('x', 'tsc', '--noEmit') },
                @{ Name = 'landing-test';  Command = 'bun'; Args = @('test') },
                @{ Name = 'landing-build'; Command = 'bun'; Args = @('run', 'build') },
                @{ Name = 'landing-smoke'; Command = 'bun'; Args = @('run', 'scripts/smoke-build.ts') }
            )
            foreach ($step in $landingSteps) {
                Write-Output "== $($step.Name)"
                & $step.Command @($step.Args)
                if ($LASTEXITCODE -ne 0) {
                    $failed += $step.Name
                }
            }
        } finally {
            Pop-Location
        }
    } else {
        Write-Output '== landing'
        Write-Output ' - bun not found on PATH (install Bun or pass -SkipLanding explicitly)'
        $failed += 'landing-bun-missing'
    }
}

if ($failed.Count -gt 0) {
    Write-Output "GATE FAILED: $($failed -join ', ')"
    exit 1
}
Write-Output 'GATE PASSED'
