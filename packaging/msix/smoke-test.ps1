[CmdletBinding()]
param(
    [string]$ExecutablePath = 'target\release\otto-os.exe',
    [ValidateRange(2, 60)]
    [int]$StartupTimeoutSeconds = 10,
    [ValidateRange(2, 30)]
    [int]$ShutdownTimeoutSeconds = 10
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$executable = Join-Path $repoRoot $ExecutablePath

if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
    throw "Executable was not found: $executable"
}

$existing = Get-CimInstance Win32_Process | Where-Object {
    $_.Name -eq 'otto-os.exe' -and $_.ExecutablePath -and
    [IO.Path]::GetFullPath($_.ExecutablePath) -eq $executable
}
if ($existing) {
    $pids = ($existing | Select-Object -ExpandProperty ProcessId) -join ','
    throw "An Otto instance is already running for this executable (PID $pids). Close it from its owner before running smoke acceptance."
}

if (-not ('OttoSmokeNative' -as [type])) {
    Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public static class OttoSmokeNative
{
    private const uint WmClose = 0x0010;
    private delegate bool EnumWindowsProc(IntPtr window, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr window);

    [DllImport("user32.dll")]
    private static extern bool PostMessage(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);

    public static IntPtr[] VisibleWindows(uint processId)
    {
        var windows = new List<IntPtr>();
        EnumWindows((window, _) =>
        {
            GetWindowThreadProcessId(window, out var owner);
            if (owner == processId && IsWindowVisible(window))
            {
                windows.Add(window);
            }
            return true;
        }, IntPtr.Zero);
        return windows.ToArray();
    }

    public static void RequestClose(IntPtr window)
    {
        PostMessage(window, WmClose, IntPtr.Zero, IntPtr.Zero);
    }
}
'@
}

$process = Start-Process -FilePath $executable -WorkingDirectory $repoRoot -PassThru -WindowStyle Hidden
$gracefulCloseRequested = $false
try {
    $startupDeadline = [DateTime]::UtcNow.AddSeconds($StartupTimeoutSeconds)
    $windows = @()
    do {
        Start-Sleep -Milliseconds 250
        $process.Refresh()
        if ($process.HasExited) {
            throw "Otto exited during startup with code $($process.ExitCode)"
        }
        $windows = [OttoSmokeNative]::VisibleWindows([uint32]$process.Id)
    } while ($windows.Count -eq 0 -and [DateTime]::UtcNow -lt $startupDeadline)

    if ($windows.Count -eq 0) {
        throw 'Otto did not create a visible top-level window before the startup deadline.'
    }

    foreach ($window in $windows) {
        [OttoSmokeNative]::RequestClose($window)
    }
    $gracefulCloseRequested = $true
    if (-not $process.WaitForExit($ShutdownTimeoutSeconds * 1000)) {
        throw 'Otto did not exit after WM_CLOSE within the shutdown deadline.'
    }
    Write-Output "SMOKE_PASS pid=$($process.Id) exit_code=$($process.ExitCode) windows=$($windows.Count)"
}
finally {
    $process.Refresh()
    if (-not $process.HasExited) {
        if (-not $gracefulCloseRequested) {
            foreach ($window in [OttoSmokeNative]::VisibleWindows([uint32]$process.Id)) {
                [OttoSmokeNative]::RequestClose($window)
            }
            $process.WaitForExit(2000) | Out-Null
        }
        if (-not $process.HasExited) {
            Stop-Process -Id $process.Id -Force
        }
    }
}
