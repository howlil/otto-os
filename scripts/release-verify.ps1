[CmdletBinding()]
param(
    [ValidateSet('all', 'full', 'success', 'false', 'edge', 'worst', 'recovery', 'fullscreen', 'genie', 'dock', 'keyboard', 'keyboard-dock', 'stall', 'appearance', 'audio', 'visual-reference', 'dock-settings', 'native-minimize', 'window-cover')]
    [string]$E2EScenario = 'full',
    [switch]$SkipIgnoredTests,
    [switch]$SkipE2E,
    [switch]$SkipMsixSmoke,
    [switch]$RequireCargoDeny
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

$runId = Get-Date -Format 'yyyyMMdd-HHmmss'
$artifactDir = Join-Path $repoRoot 'artifacts\verify'
$null = New-Item -ItemType Directory -Path $artifactDir -Force

$results = [System.Collections.Generic.List[object]]::new()
$failed = [System.Collections.Generic.List[string]]::new()

function Invoke-VerifyStep {
    param(
        [string]$Name,
        [string]$Command,
        [string[]]$Args
    )
    Write-Output "== $Name"
    $sw = [Diagnostics.Stopwatch]::StartNew()
    & $Command @Args
    $code = $LASTEXITCODE
    $sw.Stop()
    $status = if ($code -eq 0) { 'PASS' } else { 'FAIL' }
    $results.Add([ordered]@{
        name = $Name
        status = $status
        exit_code = $code
        duration_ms = $sw.ElapsedMilliseconds
    })
    if ($code -ne 0) {
        $failed.Add($Name)
    }
}

$gateArgs = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', 'scripts/gate.ps1')
if ($RequireCargoDeny) {
    $gateArgs += '-RequireCargoDeny'
}
Invoke-VerifyStep -Name 'gate' -Command 'powershell' -Args $gateArgs

if (-not $SkipIgnoredTests) {
    Invoke-VerifyStep -Name 'screen-grabber' -Command 'cargo' -Args @(
        'test', '-p', 'otto-platform',
        'the_screen_grabber_returns_a_frame_of_the_requested_size',
        '--', '--exact', '--test-threads=1'
    )
    Invoke-VerifyStep -Name 'ignored-native-and-leak-tests' -Command 'cargo' -Args @(
        'test', '--workspace', '--', '--ignored', '--test-threads=1'
    )
}

Invoke-VerifyStep -Name 'release-build' -Command 'cargo' -Args @('build', '--release', '-p', 'otto-os')

if (-not $SkipE2E) {
    Invoke-VerifyStep -Name "windows-e2e-$E2EScenario" -Command 'powershell' -Args @(
        '-NoProfile', '-ExecutionPolicy', 'Bypass',
        '-File', 'tests/e2e/windows-e2e.ps1',
        '-Scenario', $E2EScenario,
        '-ExecutablePath', 'target\release\otto-os.exe'
    )
}

if (-not $SkipMsixSmoke) {
    Invoke-VerifyStep -Name 'msix-smoke' -Command 'powershell' -Args @(
        '-NoProfile', '-ExecutionPolicy', 'Bypass',
        '-File', 'packaging/msix/smoke-test.ps1',
        '-ExecutablePath', 'target\release\otto-os.exe'
    )
}

$reportPath = Join-Path $artifactDir "release-verify-$runId.json"
$report = [ordered]@{
    run_id = $runId
    e2e_scenario = $E2EScenario
    status = if ($failed.Count -eq 0) { 'PASS' } else { 'FAIL' }
    failed = @($failed)
    steps = @($results)
}
$report | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $reportPath -Encoding utf8
Write-Output "VERIFY_REPORT=$reportPath"

if ($failed.Count -gt 0) {
    Write-Output "RELEASE VERIFY FAILED: $($failed -join ', ')"
    exit 1
}
Write-Output 'RELEASE VERIFY PASSED'
