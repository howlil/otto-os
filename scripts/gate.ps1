$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)

$steps = @(
    @{ Name = 'fmt';    Command = 'cargo'; Args = @('fmt', '--all', '--', '--check') },
    @{ Name = 'test';   Command = 'cargo'; Args = @('test', '--workspace') },
    @{ Name = 'check';  Command = 'cargo'; Args = @('check', '--workspace') },
    @{ Name = 'clippy'; Command = 'cargo'; Args = @('clippy', '--workspace', '--all-targets', '--all-features', '--', '-D', 'warnings') },
    @{ Name = 'e2e-static'; Command = 'powershell'; Args = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', 'scripts/check-e2e.ps1') }
)

$failed = @()
foreach ($step in $steps) {
    Write-Output "== $($step.Name)"
    & $step.Command @($step.Args)
    if ($LASTEXITCODE -ne 0) {
        $failed += $step.Name
    }
}

if ($failed.Count -gt 0) {
    Write-Output "GATE FAILED: $($failed -join ', ')"
    exit 1
}
Write-Output 'GATE PASSED: fmt, test, check, clippy, e2e-static'
