$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)

$steps = @(
    @{ Name = 'fmt';    Args = @('fmt', '--all', '--', '--check') },
    @{ Name = 'test';   Args = @('test', '--workspace') },
    @{ Name = 'check';  Args = @('check', '--workspace') },
    @{ Name = 'clippy'; Args = @('clippy', '--workspace', '--all-targets', '--all-features', '--', '-D', 'warnings') }
)

$failed = @()
foreach ($step in $steps) {
    Write-Output "== $($step.Name)"
    & cargo @($step.Args)
    if ($LASTEXITCODE -ne 0) {
        $failed += $step.Name
    }
}

if ($failed.Count -gt 0) {
    Write-Output "GATE FAILED: $($failed -join ', ')"
    exit 1
}
Write-Output 'GATE PASSED: fmt, test, check, clippy'
