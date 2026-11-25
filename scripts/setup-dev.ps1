$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)

Write-Output '== Configuring local Git hooks (.githooks)'
git config core.hooksPath .githooks
$hooksPath = (git config --get core.hooksPath).Trim()
if ($hooksPath -ne '.githooks') {
    throw "Expected core.hooksPath=.githooks, got '$hooksPath'"
}
Write-Output "   core.hooksPath = $hooksPath"

Write-Output '== Verifying local toolchain'
$checks = @(
    @{ Name = 'cargo';      Required = $true;  Hint = 'Install Rust toolchain matching rust-toolchain.toml' },
    @{ Name = 'rustc';      Required = $true;  Hint = 'Install Rust toolchain matching rust-toolchain.toml' },
    @{ Name = 'bun';        Required = $false; Hint = 'Install Bun (https://bun.sh) to validate landing/' },
    @{ Name = 'cargo-deny'; Required = $false; Hint = 'Install via: cargo install cargo-deny --locked' }
)

$missingRequired = @()
foreach ($check in $checks) {
    $cmd = Get-Command $check.Name -ErrorAction SilentlyContinue
    if ($cmd) {
        Write-Output ("   [OK]   {0,-12} -> {1}" -f $check.Name, $cmd.Source)
    } elseif ($check.Required) {
        Write-Output ("   [FAIL] {0,-12} -> {1}" -f $check.Name, $check.Hint)
        $missingRequired += $check.Name
    } else {
        Write-Output ("   [WARN] {0,-12} -> {1}" -f $check.Name, $check.Hint)
    }
}

Write-Output '== Checking worktree hygiene'
git worktree list

if ($missingRequired.Count -gt 0) {
    throw "Missing required tools: $($missingRequired -join ', ')"
}
Write-Output 'SETUP OK'
