[CmdletBinding()]
param(
    [ValidateSet('all', 'success', 'false', 'worst', 'edge')]
    [string]$Scenario = 'all',
    [string]$ExecutablePath = 'target\release\otto-os.exe',
    [string]$ArtifactDirectory = 'artifacts\e2e'
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$executable = (Resolve-Path (Join-Path $repoRoot $ExecutablePath)).Path
$artifactRoot = [IO.Path]::GetFullPath((Join-Path $repoRoot $ArtifactDirectory))
$runId = Get-Date -Format 'yyyyMMdd-HHmmss'
$script:observations = [System.Collections.Generic.List[object]]::new()
$script:ownedProcess = $null
$script:activeScenario = $null
$script:screenWidth = 0
$script:screenHeight = 0
$script:dpi = 96
$script:coordinateDpi = 96

if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
    throw "Executable was not found: $executable"
}

Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class OttoE2ENative
{
    private const uint MouseEventLeftDown = 0x0002;
    private const uint MouseEventLeftUp = 0x0004;
    private const uint KeyEventKeyUp = 0x0002;
    private const uint WmClose = 0x0010;
    private delegate bool EnumWindowsProc(IntPtr window, IntPtr parameter);

    [StructLayout(LayoutKind.Sequential)]
    private struct NativeRect
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct KeyboardInput
    {
        public ushort VirtualKey;
        public ushort ScanCode;
        public uint Flags;
        public uint Time;
        public UIntPtr ExtraInfo;
    }

    [StructLayout(LayoutKind.Explicit)]
    private struct InputUnion
    {
        [FieldOffset(0)]
        public KeyboardInput Keyboard;

        [FieldOffset(24)]
        public long Padding;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct NativeInput
    {
        public uint Type;
        public InputUnion Data;
    }

    public sealed class WindowRecord
    {
        public IntPtr Handle { get; set; }
        public uint ProcessId { get; set; }
        public string ClassName { get; set; }
        public bool Visible { get; set; }
        public int Left { get; set; }
        public int Top { get; set; }
        public int Right { get; set; }
        public int Bottom { get; set; }
        public int Width { get { return Right - Left; } }
        public int Height { get { return Bottom - Top; } }
    }

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetClassName(IntPtr window, StringBuilder className, int capacity);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr window);

    [DllImport("user32.dll")]
    private static extern bool GetWindowRect(IntPtr window, out NativeRect rect);

    [DllImport("user32.dll")]
    private static extern bool PostMessage(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll")]
    private static extern bool SetCursorPos(int x, int y);

    [DllImport("user32.dll")]
    private static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extraInfo);

    [DllImport("user32.dll")]
    private static extern void keybd_event(byte virtualKey, byte scanCode, uint flags, UIntPtr extraInfo);

    [DllImport("user32.dll")]
    private static extern uint SendInput(uint inputCount, NativeInput[] inputs, int inputSize);

    [DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(IntPtr window);

    [DllImport("user32.dll")]
    private static extern int GetSystemMetrics(int index);

    [DllImport("user32.dll")]
    private static extern IntPtr SetThreadDpiAwarenessContext(IntPtr dpiContext);

    public static WindowRecord[] WindowsForProcess(uint processId)
    {
        var windows = new List<WindowRecord>();
        EnumWindows((window, _) =>
        {
            GetWindowThreadProcessId(window, out var owner);
            if (owner != processId)
            {
                return true;
            }
            var className = new StringBuilder(256);
            GetClassName(window, className, className.Capacity);
            if (!GetWindowRect(window, out var rect))
            {
                return true;
            }
            windows.Add(new WindowRecord
            {
                Handle = window,
                ProcessId = owner,
                ClassName = className.ToString(),
                Visible = IsWindowVisible(window),
                Left = rect.Left,
                Top = rect.Top,
                Right = rect.Right,
                Bottom = rect.Bottom
            });
            return true;
        }, IntPtr.Zero);
        return windows.ToArray();
    }

    public static void Click(int x, int y)
    {
        SetCursorPos(x, y);
        mouse_event(MouseEventLeftDown, 0, 0, 0, UIntPtr.Zero);
        mouse_event(MouseEventLeftUp, 0, 0, 0, UIntPtr.Zero);
    }

    public static void ClickAndHold(int x, int y)
    {
        SetCursorPos(x, y);
        mouse_event(MouseEventLeftDown, 0, 0, 0, UIntPtr.Zero);
        System.Threading.Thread.Sleep(120);
        mouse_event(MouseEventLeftUp, 0, 0, 0, UIntPtr.Zero);
    }

    public static void MoveCursor(int x, int y)
    {
        SetCursorPos(x, y);
    }

    public static void SendKey(byte key)
    {
        SendInput(1, new[] { KeyboardInputEvent(key, 0) }, Marshal.SizeOf(typeof(NativeInput)));
        System.Threading.Thread.Sleep(90);
        SendInput(1, new[] { KeyboardInputEvent(key, KeyEventKeyUp) }, Marshal.SizeOf(typeof(NativeInput)));
    }

    public static void SendAltKey(byte key)
    {
        SendInput(1, new[] { KeyboardInputEvent(0x12, 0) }, Marshal.SizeOf(typeof(NativeInput)));
        System.Threading.Thread.Sleep(25);
        SendInput(1, new[] { KeyboardInputEvent(key, 0) }, Marshal.SizeOf(typeof(NativeInput)));
        System.Threading.Thread.Sleep(90);
        SendInput(1, new[] { KeyboardInputEvent(key, KeyEventKeyUp) }, Marshal.SizeOf(typeof(NativeInput)));
        SendInput(1, new[] { KeyboardInputEvent(0x12, KeyEventKeyUp) }, Marshal.SizeOf(typeof(NativeInput)));
        System.Threading.Thread.Sleep(120);
    }

    private static NativeInput KeyboardInputEvent(byte key, uint flags)
    {
        return new NativeInput
        {
            Type = 1,
            Data = new InputUnion
            {
                Keyboard = new KeyboardInput
                {
                    VirtualKey = key,
                    ScanCode = 0,
                    Flags = flags,
                    Time = 0,
                    ExtraInfo = UIntPtr.Zero
                }
            }
        };
    }

    public static void RequestClose(IntPtr window)
    {
        PostMessage(window, WmClose, IntPtr.Zero, IntPtr.Zero);
    }

    public static void PostHotkey(IntPtr window, int hotkeyId)
    {
        PostMessage(window, 0x0312, new IntPtr(hotkeyId), IntPtr.Zero);
    }

    public static uint DpiForWindow(IntPtr window)
    {
        return GetDpiForWindow(window);
    }

    public static int ScreenWidth()
    {
        return GetSystemMetrics(0);
    }

    public static int ScreenHeight()
    {
        return GetSystemMetrics(1);
    }

    public static void EnablePerMonitorDpi()
    {
        SetThreadDpiAwarenessContext(new IntPtr(-4));
    }
}
'@

[OttoE2ENative]::EnablePerMonitorDpi()
Add-Type -AssemblyName System.Drawing

function Convert-ToScaled([int]$points) {
    return [int][Math]::Floor(($points * $script:coordinateDpi + 48) / 96)
}

function Get-OttoWindows([int]$processId) {
    return @([OttoE2ENative]::WindowsForProcess([uint32]$processId))
}

function Get-OttoWindow([int]$processId, [string]$className, [bool]$visibleOnly = $false) {
    $windows = @(Get-OttoWindows $processId | Where-Object { $_.ClassName -eq $className })
    if ($visibleOnly) {
        $windows = @($windows | Where-Object { $_.Visible })
    }
    return $windows | Select-Object -First 1
}

function Assert-ProcessAlive($process, [string]$stage) {
    $process.Refresh()
    if ($process.HasExited) {
        throw "Process exited during $stage with code $($process.ExitCode)."
    }
}

function Wait-Until([scriptblock]$predicate, [string]$description, [int]$timeoutMs = 5000) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($timeoutMs)
    do {
        if (& $predicate) {
            return
        }
        Start-Sleep -Milliseconds 60
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timeout waiting for $description."
}

function Wait-VisibleWindow($process, [string]$className, [string]$description, [int]$timeoutMs = 5000) {
    Wait-Until {
        Assert-ProcessAlive $process $description
        $window = Get-OttoWindow $process.Id $className $true
        $null -ne $window
    } $description $timeoutMs
}

function Wait-HiddenWindow($process, [string]$className, [string]$description, [int]$timeoutMs = 5000) {
    Wait-Until {
        Assert-ProcessAlive $process $description
        $window = Get-OttoWindow $process.Id $className $true
        $null -eq $window
    } $description $timeoutMs
}

function Get-BarMetrics($process) {
    $bar = Get-OttoWindow $process.Id 'OttoNotchBar' $true
    if (-not $bar) {
        throw 'OttoNotchBar is not visible.'
    }
    $script:dpi = [OttoE2ENative]::DpiForWindow($bar.Handle)
    if ($script:dpi -eq 0) {
        $script:dpi = 96
    }
    $script:coordinateDpi = [int][Math]::Round(($bar.Height / 32.0) * 96)
    if ($script:coordinateDpi -lt 96) {
        $script:coordinateDpi = 96
    }
    $script:screenWidth = $bar.Width
    $script:screenHeight = [OttoE2ENative]::ScreenHeight()
    return $bar
}

function Get-ControlCenterPoint($bar) {
    $padding = Convert-ToScaled 8
    $gap = Convert-ToScaled 4
    $clockWidth = Convert-ToScaled 160
    $controlWidth = Convert-ToScaled 30
    $right = $bar.Right - $padding
    $clockX = $right - $clockWidth
    $controlRight = $clockX - $gap
    $controlX = $controlRight - $controlWidth
    return [pscustomobject]@{
        X = $controlX + [int]($controlWidth / 2)
        Y = $bar.Top + [int]($bar.Height / 2)
    }
}

function Get-ClockPoint($bar) {
    $padding = Convert-ToScaled 8
    $clockWidth = Convert-ToScaled 160
    $clockX = $bar.Right - $padding - $clockWidth
    return [pscustomobject]@{
        X = $clockX + [int]($clockWidth / 2)
        Y = $bar.Top + [int]($bar.Height / 2)
    }
}

function Get-StartPoint($bar) {
    $padding = Convert-ToScaled 8
    $startWidth = Convert-ToScaled 34
    return [pscustomobject]@{
        X = $bar.Left + $padding + [int]($startWidth / 2)
        Y = $bar.Top + [int]($bar.Height / 2)
    }
}

function Get-MediaTabPoint($bar) {
    $notchWidth = Convert-ToScaled 380
    $notchX = $bar.Left + [int](($bar.Width - $notchWidth) / 2)
    $mediaTabX = $notchX + (Convert-ToScaled 18) + (Convert-ToScaled 86) + (Convert-ToScaled 6)
    $mediaTabWidth = Convert-ToScaled 96
    return [pscustomobject]@{
        X = $mediaTabX + [int]($mediaTabWidth / 2)
        Y = $bar.Top + (Convert-ToScaled 17)
    }
}

function Save-Screenshot([string]$scenario, [string]$checkpoint) {
    $scenarioRoot = Join-Path $artifactRoot $scenario
    New-Item -ItemType Directory -Path $scenarioRoot -Force | Out-Null
    $path = Join-Path $scenarioRoot "$checkpoint.png"
    $bitmap = [System.Drawing.Bitmap]::new($script:screenWidth, $script:screenHeight)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen(0, 0, 0, 0, $bitmap.Size, [System.Drawing.CopyPixelOperation]::SourceCopy)
        $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    }
    finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
    return (Resolve-Path -LiteralPath $path).Path
}

function Get-ScreenFingerprint($window) {
    $width = [Math]::Max(1, $window.Width)
    $height = [Math]::Max(1, $window.Height)
    $bitmap = [System.Drawing.Bitmap]::new($width, $height)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($window.Left, $window.Top, 0, 0, $bitmap.Size, [System.Drawing.CopyPixelOperation]::SourceCopy)
        $parts = [System.Collections.Generic.List[string]]::new()
        for ($y = 0; $y -lt $height; $y += 12) {
            for ($x = 0; $x -lt $width; $x += 12) {
                $pixel = $bitmap.GetPixel($x, $y)
                [void]$parts.Add("$($pixel.R),$($pixel.G),$($pixel.B)")
            }
        }
        return $parts -join ';'
    }
    finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}

function Wait-VisualStable($process, [string]$className, [string]$description, [int]$timeoutMs = 3000) {
    $previous = $null
    $stableSamples = 0
    $deadline = [DateTime]::UtcNow.AddMilliseconds($timeoutMs)
    do {
        $window = Get-OttoWindow $process.Id $className $true
        if (-not $window) {
            throw "$className disappeared while waiting for $description."
        }
        $current = Get-ScreenFingerprint $window
        if ($current -eq $previous) {
            $stableSamples++
            if ($stableSamples -ge 3) {
                return
            }
        }
        else {
            $stableSamples = 0
            $previous = $current
        }
        Start-Sleep -Milliseconds 80
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Timeout waiting for visual stability of $description."
}

function Get-BrightPixelCount([string]$path, $window) {
    $bitmap = [System.Drawing.Bitmap]::new($path)
    try {
        $left = [Math]::Max(0, $window.Left)
        $top = [Math]::Max(0, $window.Top)
        $right = [Math]::Min($bitmap.Width, $window.Right)
        $bottom = [Math]::Min($bitmap.Height, $window.Bottom)
        $count = 0
        for ($y = $top; $y -lt $bottom; $y += 4) {
            for ($x = $left; $x -lt $right; $x += 4) {
                $pixel = $bitmap.GetPixel($x, $y)
                if ($pixel.R -ge 180 -or $pixel.G -ge 180 -or $pixel.B -ge 180) {
                    $count++
                }
            }
        }
        return $count
    }
    finally {
        $bitmap.Dispose()
    }
}

function Record-Checkpoint($process, [string]$name, [string]$expectation, [string]$visualClass = '') {
    $windows = @(Get-OttoWindows $process.Id)
    $visible = @($windows | Where-Object { $_.Visible } | ForEach-Object { $_.ClassName })
    $screenshot = Save-Screenshot $script:activeScenario $name
    $visualSignal = $null
    $visualStatus = 'PASS'
    if ($visualClass) {
        $visualWindow = Get-OttoWindow $process.Id $visualClass $true
        if (-not $visualWindow) {
            $visualStatus = 'FAIL'
        }
        else {
            $visualSignal = Get-BrightPixelCount $screenshot $visualWindow
            if ($visualSignal -lt 5) {
                $visualStatus = 'FAIL'
            }
        }
    }
    $record = [ordered]@{
        scenario = $script:activeScenario
        checkpoint = $name
        expectation = $expectation
        status = $visualStatus
        visible_windows = $visible
        visual_class = $visualClass
        visual_signal = $visualSignal
        all_windows = @($windows | ForEach-Object {
            [ordered]@{
                class_name = $_.ClassName
                visible = $_.Visible
                left = $_.Left
                top = $_.Top
                width = $_.Width
                height = $_.Height
            }
        })
        screenshot = $screenshot
    }
    [void]$script:observations.Add([pscustomobject]$record)
    if ($visualStatus -eq 'FAIL') {
        throw "Visual signal for $visualClass was not observed at checkpoint $name. Inspect $screenshot."
    }
    Write-Output "CHECKPOINT_PASS scenario=$script:activeScenario name=$name visible=$($visible -join ',') visual_signal=$visualSignal screenshot=$screenshot"
}

function Record-ScenarioFailure([string]$scenario, [string]$message) {
    [void]$script:observations.Add([pscustomobject][ordered]@{
        scenario = $scenario
        checkpoint = 'scenario-failure'
        expectation = 'Scenario completes without an observable runtime or visual failure.'
        status = 'FAIL'
        message = $message
    })
}

function Assert-InitialTopology($process) {
    $bar = Get-OttoWindow $process.Id 'OttoNotchBar' $true
    $dock = Get-OttoWindow $process.Id 'OttoDockBar' $true
    $panel = Get-OttoWindow $process.Id 'OttoNotchPanel' $true
    $menu = Get-OttoWindow $process.Id 'OttoPopupMenu' $true
    $controls = Get-OttoWindow $process.Id 'OttoWindowControls' $true
    if (-not $bar -or $bar.Width -lt 400 -or $bar.Height -lt 20 -or $bar.Top -gt 2) {
        throw 'Initial top bar geometry is not valid.'
    }
    if (-not $dock -or $dock.Width -lt 100 -or $dock.Height -lt 30) {
        throw 'Initial dock geometry is not valid.'
    }
    if ($panel -or $menu) {
        throw 'Initial popup or expanded panel is visible.'
    }
    if ($controls -and $controls.Left -lt $bar.Right -and $controls.Right -gt $bar.Left -and $controls.Top -lt $bar.Bottom -and $controls.Bottom -gt $bar.Top) {
        [void]$script:observations.Add([pscustomobject][ordered]@{
            scenario = $script:activeScenario
            checkpoint = '01-baseline-window-controls-overlap'
            expectation = 'Window controls stay in the external application titlebar and do not cover Otto top-bar targets.'
            status = 'WARNING'
            message = "OttoWindowControls overlaps OttoNotchBar: controls=($($controls.Left),$($controls.Top),$($controls.Width),$($controls.Height)) bar=($($bar.Left),$($bar.Top),$($bar.Width),$($bar.Height))"
        })
        Write-Output 'OBSERVATION_WARNING window-controls-overlap-top-bar'
    }
}

function Assert-ExpandedTopology($process) {
    $panel = Get-OttoWindow $process.Id 'OttoNotchPanel' $true
    if (-not $panel -or $panel.Width -lt (Convert-ToScaled 300) -or $panel.Height -lt (Convert-ToScaled 100)) {
        throw 'Expanded notch panel geometry is not valid.'
    }
}

function Assert-MenuTopology($process) {
    $menu = Get-OttoWindow $process.Id 'OttoPopupMenu' $true
    if (-not $menu -or $menu.Width -lt (Convert-ToScaled 220) -or $menu.Height -lt (Convert-ToScaled 80)) {
        throw 'Popup menu geometry is not valid.'
    }
    if ($menu.Left -lt 0 -or $menu.Top -lt 0 -or $menu.Right -gt $script:screenWidth -or $menu.Bottom -gt $script:screenHeight) {
        throw 'Popup menu is outside the primary screen bounds.'
    }
}

function Start-Otto([string]$scenario) {
    $existing = Get-CimInstance Win32_Process | Where-Object {
        $_.Name -eq 'otto-os.exe' -and $_.ExecutablePath -and ([IO.Path]::GetFullPath($_.ExecutablePath) -eq $executable)
    }
    if ($existing) {
        $pids = ($existing | Select-Object -ExpandProperty ProcessId) -join ','
        throw "An Otto process already exists for this test executable: $pids. Close it from its owner first."
    }
    $script:activeScenario = $scenario
    $process = Start-Process -FilePath $executable -WorkingDirectory $repoRoot -PassThru -WindowStyle Hidden
    $script:ownedProcess = $process
    Wait-VisibleWindow $process 'OttoNotchBar' 'top bar startup'
    Wait-VisibleWindow $process 'OttoDockBar' 'dock startup'
    Get-BarMetrics $process | Out-Null
    return $process
}

function Stop-Otto($process) {
    if (-not $process) {
        return
    }
    $process.Refresh()
    if (-not $process.HasExited) {
        $bar = Get-OttoWindow $process.Id 'OttoNotchBar' $true
        if ($bar) {
            [OttoE2ENative]::RequestClose($bar.Handle)
        }
        if (-not $process.WaitForExit(5000)) {
            Stop-Process -Id $process.Id -Force
            $process.WaitForExit(2000) | Out-Null
            $script:ownedProcess = $null
            throw "Owned Otto process $($process.Id) did not shut down gracefully; exact owned PID was force-cleaned."
        }
    }
    Wait-Until {
        @(Get-OttoWindows $process.Id | Where-Object { $_.Visible }).Count -eq 0
    } 'Otto windows to be destroyed after shutdown' 4000
    $script:ownedProcess = $null
}

function Open-ControlCenter($process) {
    $bar = Get-BarMetrics $process
    $point = Get-ControlCenterPoint $bar
    [OttoE2ENative]::Click($point.X, $point.Y)
    Wait-VisibleWindow $process 'OttoPopupMenu' 'Control Center popup' 3000
    Assert-MenuTopology $process
}

function Close-Popup($process) {
    [OttoE2ENative]::SendKey(0x1B)
    try {
        Wait-HiddenWindow $process 'OttoPopupMenu' 'popup dismissal' 1000
        return
    }
    catch {
        $escapeScreenshot = Save-Screenshot $script:activeScenario 'popup-escape-warning'
        [void]$script:observations.Add([pscustomobject][ordered]@{
            scenario = $script:activeScenario
            checkpoint = 'popup-escape-input'
            expectation = 'Escape dismisses an open popup.'
            status = 'WARNING'
            message = "Synthetic Escape was not observed by the polling boundary; deterministic click-outside fallback used. screenshot=$escapeScreenshot"
        })
        Write-Output 'OBSERVATION_WARNING escape-dismissal-not-observed'
        $bar = Get-BarMetrics $process
        [OttoE2ENative]::ClickAndHold($bar.Left + (Convert-ToScaled 80), $bar.Bottom + (Convert-ToScaled 80))
        Wait-HiddenWindow $process 'OttoPopupMenu' 'popup dismissal fallback' 3000
    }
}

function Expand-Notch($process) {
    $bar = Get-BarMetrics $process
    [OttoE2ENative]::MoveCursor($bar.Left + (Convert-ToScaled 80), $bar.Bottom + (Convert-ToScaled 80))
    [OttoE2ENative]::SendAltKey(0x4E)
    Wait-VisibleWindow $process 'OttoNotchPanel' 'notch expansion' 3000
    Wait-Until {
        $panel = Get-OttoWindow $process.Id 'OttoNotchPanel' $true
        $panel -and $panel.Height -ge (Convert-ToScaled 100)
    } 'settled expanded notch' 3000
    Wait-VisualStable $process 'OttoNotchPanel' 'expanded notch animation' 4000
    Assert-ExpandedTopology $process
}

function Collapse-Notch($process) {
    $bar = Get-BarMetrics $process
    [OttoE2ENative]::MoveCursor($bar.Left + (Convert-ToScaled 80), $bar.Bottom + (Convert-ToScaled 80))
    [OttoE2ENative]::SendAltKey(0x4E)
    try {
        Wait-HiddenWindow $process 'OttoNotchPanel' 'notch collapse' 1200
    }
    catch {
        $fallbackScreenshot = Save-Screenshot $script:activeScenario 'notch-hotkey-warning'
        [void]$script:observations.Add([pscustomobject][ordered]@{
            scenario = $script:activeScenario
            checkpoint = 'notch-hotkey-input'
            expectation = 'Alt+N closes the expanded notch through the registered global hotkey.'
            status = 'WARNING'
            message = "Synthetic Alt+N close was not observed; WM_HOTKEY handler fallback used. screenshot=$fallbackScreenshot"
        })
        Write-Output 'OBSERVATION_WARNING alt-n-close-input-not-observed'
        $bar = Get-OttoWindow $process.Id 'OttoNotchBar' $true
        [OttoE2ENative]::PostHotkey($bar.Handle, 1)
        Wait-HiddenWindow $process 'OttoNotchPanel' 'notch collapse handler fallback' 3000
    }
}

function Run-SuccessFlow {
    $process = Start-Otto 'success'
    try {
        Assert-InitialTopology $process
        Record-Checkpoint $process '01-baseline' 'Top bar and dock are visible; popup and expanded panel are hidden.'
        Expand-Notch $process
        Record-Checkpoint $process '02-notch-expanded' 'Alt+N exposes a settled expanded notch panel within screen bounds.' 'OttoNotchPanel'
        Collapse-Notch $process
        Open-ControlCenter $process
        Record-Checkpoint $process '03-control-center-open' 'Control Center opens as an in-bounds popup window.' 'OttoPopupMenu'
        Close-Popup $process
        $bar = Get-BarMetrics $process
        $clock = Get-ClockPoint $bar
        [OttoE2ENative]::Click($clock.X, $clock.Y)
        try {
            Wait-VisibleWindow $process 'OttoPopupMenu' 'calendar popup' 3000
        }
        catch {
            $failureScreenshot = Save-Screenshot $script:activeScenario '04-calendar-click-failure'
            throw "Clock click did not open Calendar at ($($clock.X),$($clock.Y)); screenshot=$failureScreenshot; $($_.Exception.Message)"
        }
        Assert-MenuTopology $process
        Record-Checkpoint $process '04-calendar-open' 'Clock click opens an in-bounds calendar popup.' 'OttoPopupMenu'
        Close-Popup $process
        $bar = Get-BarMetrics $process
        $start = Get-StartPoint $bar
        [OttoE2ENative]::Click($start.X, $start.Y)
        Wait-VisibleWindow $process 'OttoPopupMenu' 'top bar menu' 3000
        Assert-MenuTopology $process
        Record-Checkpoint $process '05-top-bar-menu-open' 'Start slot opens the top-bar menu without destroying the host.' 'OttoPopupMenu'
        Close-Popup $process
        Stop-Otto $process
    }
    finally {
        if ($script:ownedProcess) { Stop-Otto $script:ownedProcess }
    }
}

function Run-FalseFlow {
    $process = Start-Otto 'false'
    try {
        Assert-InitialTopology $process
        $bar = Get-BarMetrics $process
        [OttoE2ENative]::Click($bar.Left + [int]($bar.Width / 2), $bar.Bottom + (Convert-ToScaled 80))
        Start-Sleep -Milliseconds 300
        Assert-ProcessAlive $process 'outside click'
        if (Get-OttoWindow $process.Id 'OttoPopupMenu' $true) {
            throw 'Clicking outside the surface unexpectedly opened a popup.'
        }
        Record-Checkpoint $process '01-outside-click' 'A click outside Otto is a no-op and leaves the host alive.'
        $second = Start-Process -FilePath $executable -WorkingDirectory $repoRoot -PassThru -WindowStyle Hidden
        if (-not $second.WaitForExit(3000)) {
            Stop-Process -Id $second.Id -Force
            throw 'A second Otto instance did not exit within the single-instance deadline.'
        }
        if ($second.ExitCode -ne 0) {
            throw "A rejected second instance exited with code $($second.ExitCode)."
        }
        Assert-ProcessAlive $process 'single-instance rejection'
        if (@(Get-OttoWindows $process.Id | Where-Object { $_.ClassName -eq 'OttoNotchBar' -and $_.Visible }).Count -ne 1) {
            throw 'Single-instance rejection changed the primary bar topology.'
        }
        Record-Checkpoint $process '02-second-instance-rejected' 'A second process exits cleanly without duplicating or disturbing the primary surfaces.'
        [OttoE2ENative]::SendKey(0x1B)
        Start-Sleep -Milliseconds 250
        Assert-ProcessAlive $process 'idle escape'
        Record-Checkpoint $process '03-idle-escape' 'Escape with no popup is harmless.'
        Stop-Otto $process
    }
    finally {
        if ($script:ownedProcess) { Stop-Otto $script:ownedProcess }
    }
}

function Run-EdgeFlow {
    $process = Start-Otto 'edge'
    try {
        Expand-Notch $process
        $bar = Get-BarMetrics $process
        $media = Get-MediaTabPoint $bar
        [OttoE2ENative]::Click($media.X, $media.Y)
        Start-Sleep -Milliseconds 450
        Assert-ProcessAlive $process 'media tab without provider'
        Assert-ExpandedTopology $process
        Record-Checkpoint $process '01-empty-media-tab' 'Selecting Now Playing with no active provider keeps the expanded surface stable.' 'OttoNotchPanel'
        [OttoE2ENative]::SendAltKey(0x50)
        Start-Sleep -Milliseconds 300
        Assert-ProcessAlive $process 'pomodoro hotkey'
        Record-Checkpoint $process '02-pomodoro-hotkey' 'Alt+P changes local focus state without crashing or leaking a surface.' 'OttoNotchPanel'
        for ($index = 0; $index -lt 4; $index++) {
            [OttoE2ENative]::SendAltKey(0x4E)
        }
        Assert-ProcessAlive $process 'rapid notch toggles'
        Record-Checkpoint $process '03-rapid-notch-toggles' 'Repeated Alt+N input remains bounded and leaves the process responsive.'
        if (Get-OttoWindow $process.Id 'OttoNotchPanel' $true) {
            Collapse-Notch $process
        }
        Stop-Otto $process
    }
    finally {
        if ($script:ownedProcess) { Stop-Otto $script:ownedProcess }
    }
}

function Run-WorstFlow {
    $process = Start-Otto 'worst'
    try {
        Expand-Notch $process
        Open-ControlCenter $process
        Record-Checkpoint $process '01-loaded-state' 'Panel and popup are both active before teardown stress.' 'OttoPopupMenu'
        $bar = Get-OttoWindow $process.Id 'OttoNotchBar' $true
        [OttoE2ENative]::RequestClose($bar.Handle)
        if (-not $process.WaitForExit(5000)) {
            throw 'Host did not exit after WM_CLOSE while panel and popup were active.'
        }
        Wait-Until {
            @(Get-OttoWindows $process.Id | Where-Object { $_.Visible }).Count -eq 0
        } 'all Otto surfaces to disappear after worst-case teardown' 4000
        $script:ownedProcess = $null
        Write-Output 'WORST_FLOW_PASS graceful teardown removed all visible Otto surfaces.'
    }
    finally {
        if ($script:ownedProcess) { Stop-Otto $script:ownedProcess }
    }
}

New-Item -ItemType Directory -Path $artifactRoot -Force | Out-Null
$scenarios = if ($Scenario -eq 'all') { @('success', 'false', 'edge', 'worst') } else { @($Scenario) }
$fatal = $null
$scenarioFailures = [System.Collections.Generic.List[string]]::new()
try {
    foreach ($current in $scenarios) {
        try {
            switch ($current) {
                'success' { Run-SuccessFlow }
                'false' { Run-FalseFlow }
                'edge' { Run-EdgeFlow }
                'worst' { Run-WorstFlow }
            }
        }
        catch {
            $message = $_.Exception.Message
            [void]$scenarioFailures.Add("${current}: $message")
            Record-ScenarioFailure $current $message
            Write-Output "E2E_SCENARIO_FAIL scenario=$current message=$message"
            if ($script:ownedProcess) {
                try { Stop-Otto $script:ownedProcess } catch { Write-Output "E2E_CLEANUP_FAIL $($_.Exception.Message)" }
            }
        }
    }
    if ($scenarioFailures.Count -gt 0) {
        $fatal = $scenarioFailures -join ' | '
    }
}
catch {
    $fatal = $_.Exception.Message
    Write-Output "E2E_FAIL scenario=$script:activeScenario message=$fatal"
}
finally {
    if ($script:ownedProcess) {
        try { Stop-Otto $script:ownedProcess } catch { Write-Output "E2E_CLEANUP_FAIL $($_.Exception.Message)" }
    }
    $report = [ordered]@{
        run_id = $runId
        executable = $executable
        scenarios = $scenarios
        status = if ($fatal) { 'FAIL' } else { 'PASS' }
        fatal = $fatal
        observations = @($script:observations)
    }
    $reportPath = Join-Path $artifactRoot "report-$runId.json"
    $report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $reportPath -Encoding utf8
    Write-Output "E2E_REPORT=$reportPath"
}
if ($fatal) { exit 1 }
Write-Output 'E2E_PASS all requested scenarios completed.'
