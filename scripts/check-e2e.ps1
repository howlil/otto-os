param([string]$ScriptPath, [string]$ManifestPath)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $ScriptPath) { $ScriptPath = Join-Path $root 'tests\e2e\windows-e2e.ps1' }
if (-not $ManifestPath) { $ManifestPath = Join-Path $root 'tests\visual\reference\manifest.json' }
$failures = [System.Collections.Generic.List[string]]::new()

$tokens = $null
$errors = $null
[void][System.Management.Automation.Language.Parser]::ParseFile($ScriptPath, [ref]$tokens, [ref]$errors)
foreach ($e in $errors) { $failures.Add("parse: $($e.Message)") }

$text = Get-Content -Raw -LiteralPath $ScriptPath

$validateSet = [regex]::Match($text, 'ValidateSet\(([^)]*)\)\]\s*\r?\n\s*\[string\]\$Scenario').Groups[1].Value
$scenarios = @([regex]::Matches($validateSet, "'([^']+)'") | ForEach-Object { $_.Groups[1].Value })
if ($scenarios.Count -eq 0) { $failures.Add('could not read the Scenario ValidateSet') }

$allLine = [regex]::Match($text, "\`$scenarios = if \(\`$Scenario -eq 'all'\) \{ @\(([^)]*)\) \}").Groups[1].Value
$allScenarios = @([regex]::Matches($allLine, "'([^']+)'") | ForEach-Object { $_.Groups[1].Value })

$dispatchBlock = [regex]::Match($text, 'switch \(\$current\) \{(.*?)\n\s{12}\}', 'Singleline').Groups[1].Value
foreach ($name in ($scenarios | Where-Object { $_ -ne 'all' })) {
    $dispatch = [regex]::Match($dispatchBlock, "'$([regex]::Escape($name))'\s*\{\s*(Run-[A-Za-z]+)\s*\}")
    if (-not $dispatch.Success) {
        $failures.Add("scenario '$name' has no dispatch case")
        continue
    }
    $function = $dispatch.Groups[1].Value
    if ($text -notmatch "function $function\b") {
        $failures.Add("scenario '$name' dispatches to $function, which is not defined")
    }
}

foreach ($name in $allScenarios) {
    if ($scenarios -notcontains $name) {
        $failures.Add("'all' includes '$name', which is not in the Scenario ValidateSet")
    }
}

$manifest = $null
try {
    $manifest = Get-Content -Raw -LiteralPath $ManifestPath | ConvertFrom-Json
}
catch {
    $failures.Add("manifest does not parse: $($_.Exception.Message)")
}
if ($manifest) {
    foreach ($key in 'version', 'tolerance', 'spreadMax', 'modes') {
        if ($null -eq $manifest.$key) { $failures.Add("manifest is missing '$key'") }
    }
    foreach ($mode in 'dark-opaque', 'light-opaque') {
        $modeEntry = $manifest.modes.$mode
        if ($null -eq $modeEntry) {
            $failures.Add("manifest has no mode '$mode'")
            continue
        }
        $offset = @($modeEntry.'dock-pill-offset')
        if ($offset.Count -ne 2) {
            $failures.Add("$mode/dock-pill-offset must have two values")
        }
        $hint = @($modeEntry.'dock-fill-hint')
        if ($hint.Count -ne 3) {
            $failures.Add("$mode/dock-fill-hint must have three channels")
        }
        $menuHint = @($modeEntry.'menu-fill-hint')
        if ($menuHint.Count -ne 3) {
            $failures.Add("$mode/menu-fill-hint must have three channels")
        }
        $menuOffset = @($modeEntry.'menu-fill-offset')
        if ($menuOffset.Count -ne 2) {
            $failures.Add("$mode/menu-fill-offset must have two values")
        }
        foreach ($surface in 'bar-empty', 'notch-core', 'dock-pill', 'menu-fill') {
            $value = $modeEntry.$surface
            if ($null -eq $value) { continue }
            $channels = @($value)
            if ($channels.Count -ne 3) {
                $failures.Add("$mode/$surface must be null or three channels")
                continue
            }
            foreach ($channel in $channels) {
                if ($channel -lt 0 -or $channel -gt 255) {
                    $failures.Add("$mode/$surface has channel $channel outside 0..255")
                }
            }
        }
    }
}

if ($failures.Count -gt 0) {
    Write-Output 'E2E CHECK FAILED'
    foreach ($failure in $failures) { Write-Output " - $failure" }
    exit 1
}
Write-Output "E2E CHECK PASSED: $($scenarios.Count) scenarios, $($allScenarios.Count) in all, manifest valid"
