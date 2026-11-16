[CmdletBinding()]
param(
    [ValidateSet('all', 'success', 'false', 'worst', 'edge', 'recovery', 'fullscreen', 'genie', 'dock', 'keyboard', 'keyboard-dock', 'stall', 'appearance', 'audio', 'visual-reference', 'dock-settings', 'native-minimize', 'window-cover')]
    [string]$Scenario = 'all',
    [string]$ExecutablePath = 'target\release\otto-os.exe',
    [string]$ArtifactDirectory = 'artifacts\e2e',
    [switch]$RecordReference
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
$script:desktopCaptureAvailable = $true

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
    private const uint WmLbuttonDown = 0x0201;
    private const uint WmLbuttonUp = 0x0202;
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
            uint owner;
            GetWindowThreadProcessId(window, out owner);
            if (owner != processId)
            {
                return true;
            }
            var className = new StringBuilder(256);
            GetClassName(window, className, className.Capacity);
            NativeRect rect;
            if (!GetWindowRect(window, out rect))
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
        System.Threading.Thread.Sleep(30);
        mouse_event(MouseEventLeftDown, 0, 0, 0, UIntPtr.Zero);
        System.Threading.Thread.Sleep(50);
        mouse_event(MouseEventLeftUp, 0, 0, 0, UIntPtr.Zero);
    }

    public static void ClickWindow(IntPtr window, int x, int y)
    {
        SetCursorPos(x, y);
        IntPtr lParam = new IntPtr((y << 16) | (x & 0xFFFF));
        PostMessage(window, WmLbuttonDown, new IntPtr(1), lParam);
        System.Threading.Thread.Sleep(50);
        PostMessage(window, WmLbuttonUp, IntPtr.Zero, lParam);
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

    public static void SendCtrlAltKey(byte key)
    {
        SendInput(1, new[] { KeyboardInputEvent(0x11, 0) }, Marshal.SizeOf(typeof(NativeInput)));
        SendInput(1, new[] { KeyboardInputEvent(0x12, 0) }, Marshal.SizeOf(typeof(NativeInput)));
        System.Threading.Thread.Sleep(25);
        SendInput(1, new[] { KeyboardInputEvent(key, 0) }, Marshal.SizeOf(typeof(NativeInput)));
        System.Threading.Thread.Sleep(90);
        SendInput(1, new[] { KeyboardInputEvent(key, KeyEventKeyUp) }, Marshal.SizeOf(typeof(NativeInput)));
        SendInput(1, new[] { KeyboardInputEvent(0x12, KeyEventKeyUp) }, Marshal.SizeOf(typeof(NativeInput)));
        SendInput(1, new[] { KeyboardInputEvent(0x11, KeyEventKeyUp) }, Marshal.SizeOf(typeof(NativeInput)));
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

Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class OttoE2EShell
{
    private const uint SpiGetWorkArea = 0x0030;
    private const uint AbmGetState = 0x00000004;

    [StructLayout(LayoutKind.Sequential)]
    private struct NativeRect
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct AppBarData
    {
        public uint Size;
        public IntPtr Window;
        public uint CallbackMessage;
        public uint Edge;
        public NativeRect Bounds;
        public IntPtr Param;
    }

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool SystemParametersInfo(uint action, uint param, ref NativeRect rect, uint flags);

    [DllImport("shell32.dll")]
    private static extern UIntPtr SHAppBarMessage(uint message, ref AppBarData data);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr FindWindow(string className, string windowName);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr window);

    public static bool TaskbarVisible()
    {
        var tray = FindWindow("Shell_TrayWnd", null);
        return tray != IntPtr.Zero && IsWindowVisible(tray);
    }

    public static string WorkArea()
    {
        var rect = new NativeRect();
        SystemParametersInfo(SpiGetWorkArea, 0, ref rect, 0);
        return rect.Left + "," + rect.Top + "," + rect.Right + "," + rect.Bottom;
    }

    public static bool TaskbarAutoHide()
    {
        var data = new AppBarData();
        data.Size = (uint)Marshal.SizeOf(typeof(AppBarData));
        return (SHAppBarMessage(AbmGetState, ref data).ToUInt64() & 1UL) != 0;
    }
}
'@

[OttoE2ENative]::EnablePerMonitorDpi()
Add-Type -AssemblyName System.Drawing

Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;

public static class OttoE2ESystem
{
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
    [DllImport("kernel32.dll")] private static extern IntPtr OpenThread(uint access, bool inherit, uint threadId);
    [DllImport("kernel32.dll")] private static extern uint SuspendThread(IntPtr thread);
    [DllImport("kernel32.dll")] private static extern uint ResumeThread(IntPtr thread);
    [DllImport("kernel32.dll")] private static extern bool CloseHandle(IntPtr handle);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr SendMessageTimeout(IntPtr window, uint message, UIntPtr wParam, string lParam, uint flags, uint timeout, out UIntPtr result);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr SendMessage(IntPtr window, uint message, IntPtr wParam, StringBuilder lParam);
    [DllImport("user32.dll")] private static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] private static extern void keybd_event(byte key, byte scan, uint flags, UIntPtr extra);
    private delegate bool EnumProc(IntPtr window, IntPtr parameter);
    [DllImport("user32.dll")] private static extern bool EnumChildWindows(IntPtr parent, EnumProc proc, IntPtr parameter);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetClassName(IntPtr window, StringBuilder name, int max);
    [DllImport("user32.dll")] private static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] private static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);

    private const uint ThreadSuspendResume = 0x0002;
    private const uint KeyUp = 0x0002;

    public static IntPtr SuspendWindowThread(IntPtr window)
    {
        uint processId;
        uint threadId = GetWindowThreadProcessId(window, out processId);
        IntPtr thread = OpenThread(ThreadSuspendResume, false, threadId);
        if (thread != IntPtr.Zero && SuspendThread(thread) == uint.MaxValue)
        {
            CloseHandle(thread);
            return IntPtr.Zero;
        }
        return thread;
    }

    public static void ResumeAndClose(IntPtr thread)
    {
        if (thread == IntPtr.Zero) return;
        ResumeThread(thread);
        CloseHandle(thread);
    }

    public static void BroadcastSettingChange(string area)
    {
        UIntPtr result;
        SendMessageTimeout(new IntPtr(0xffff), 0x001A, UIntPtr.Zero, area, 0x0002, 3000, out result);
    }

    public static void Focus(IntPtr window)
    {
        SetForegroundWindow(window);
    }

    public static void Chord(byte modifier, byte key)
    {
        keybd_event(modifier, 0, 0, UIntPtr.Zero);
        keybd_event(key, 0, 0, UIntPtr.Zero);
        keybd_event(key, 0, KeyUp, UIntPtr.Zero);
        keybd_event(modifier, 0, KeyUp, UIntPtr.Zero);
    }

    public static void RightClick(int x, int y)
    {
        SetCursorPos(x, y);
        System.Threading.Thread.Sleep(40);
        mouse_event(0x0008, 0, 0, 0, UIntPtr.Zero);
        System.Threading.Thread.Sleep(40);
        mouse_event(0x0010, 0, 0, 0, UIntPtr.Zero);
    }

    [DllImport("user32.dll")] private static extern bool ShowWindow(IntPtr window, int command);

    public static void Restore(IntPtr window)
    {
        ShowWindow(window, 9);
        SetForegroundWindow(window);
    }

    public static void LeftDown(int x, int y)
    {
        SetCursorPos(x, y);
        System.Threading.Thread.Sleep(40);
        mouse_event(0x0002, 0, 0, 0, UIntPtr.Zero);
    }

    public static void DragTo(int fromX, int fromY, int x, int y, int steps)
    {
        for (int step = 1; step <= steps; step++)
        {
            SetCursorPos(fromX + (x - fromX) * step / steps, fromY + (y - fromY) * step / steps);
            System.Threading.Thread.Sleep(15);
        }
    }

    public static void LeftUp()
    {
        mouse_event(0x0004, 0, 0, 0, UIntPtr.Zero);
    }

    public static void HoldKey(byte key, int milliseconds)
    {
        keybd_event(key, 0, 0, UIntPtr.Zero);
        System.Threading.Thread.Sleep(milliseconds);
        keybd_event(key, 0, KeyUp, UIntPtr.Zero);
    }

    public static void Type(string text)
    {
        foreach (char ch in text.ToUpperInvariant())
        {
            keybd_event((byte)ch, 0, 0, UIntPtr.Zero);
            keybd_event((byte)ch, 0, KeyUp, UIntPtr.Zero);
        }
    }

    public static IntPtr FindChildByClassPrefix(IntPtr parent, string prefix)
    {
        IntPtr found = IntPtr.Zero;
        EnumChildWindows(parent, (window, _) =>
        {
            var name = new StringBuilder(256);
            GetClassName(window, name, name.Capacity);
            if (name.ToString().StartsWith(prefix, StringComparison.OrdinalIgnoreCase)) { found = window; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }

    public static string WindowText(IntPtr window)
    {
        var text = new StringBuilder(512);
        SendMessage(window, 0x000D, new IntPtr(text.Capacity), text);
        return text.ToString();
    }
}

[ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")]
public class OttoE2EDeviceEnumerator { }

[ComImport, Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IOttoE2EDeviceEnumerator
{
    int EnumAudioEndpoints(int dataFlow, int stateMask, out IOttoE2EDeviceCollection devices);
    int GetDefaultAudioEndpoint(int dataFlow, int role, out IOttoE2EDevice device);
}

[ComImport, Guid("0BD7A1BE-7A1A-44DB-8397-CC5392387B5E"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IOttoE2EDeviceCollection
{
    int GetCount(out uint count);
}

[ComImport, Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IOttoE2EDevice
{
    int Activate(ref Guid iid, int context, IntPtr parameters, [MarshalAs(UnmanagedType.IUnknown)] out object endpoint);
}

[ComImport, Guid("5CDF2C82-841E-4546-9722-0CF74078229A"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IOttoE2EEndpointVolume
{
    int RegisterControlChangeNotify(IntPtr notify);
    int UnregisterControlChangeNotify(IntPtr notify);
    int GetChannelCount(out uint count);
    int SetMasterVolumeLevel(float decibels, ref Guid context);
    int SetMasterVolumeLevelScalar(float level, ref Guid context);
    int GetMasterVolumeLevel(out float decibels);
    int GetMasterVolumeLevelScalar(out float level);
}

public static class OttoE2EAudio
{
    private static IOttoE2EEndpointVolume Endpoint()
    {
        var enumerator = (IOttoE2EDeviceEnumerator)new OttoE2EDeviceEnumerator();
        IOttoE2EDevice device;
        Marshal.ThrowExceptionForHR(enumerator.GetDefaultAudioEndpoint(0, 0, out device));
        Guid iid = typeof(IOttoE2EEndpointVolume).GUID;
        object endpoint;
        Marshal.ThrowExceptionForHR(device.Activate(ref iid, 1, IntPtr.Zero, out endpoint));
        return (IOttoE2EEndpointVolume)endpoint;
    }

    public static uint ActiveRenderDevices()
    {
        var enumerator = (IOttoE2EDeviceEnumerator)new OttoE2EDeviceEnumerator();
        IOttoE2EDeviceCollection devices;
        Marshal.ThrowExceptionForHR(enumerator.EnumAudioEndpoints(0, 1, out devices));
        uint count;
        Marshal.ThrowExceptionForHR(devices.GetCount(out count));
        return count;
    }

    public static float Volume()
    {
        float level;
        Marshal.ThrowExceptionForHR(Endpoint().GetMasterVolumeLevelScalar(out level));
        return level;
    }

    public static void SetVolume(float level)
    {
        Guid context = Guid.Empty;
        Marshal.ThrowExceptionForHR(Endpoint().SetMasterVolumeLevelScalar(level, ref context));
    }
}
'@

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

function Wait-Until([scriptblock]$predicate, [string]$description, [int]$timeoutMs = 5000, [scriptblock]$Detail = $null) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($timeoutMs)
    do {
        if (& $predicate) {
            return
        }
        Start-Sleep -Milliseconds 60
    } while ([DateTime]::UtcNow -lt $deadline)
    $suffix = if ($Detail) { " ($(& $Detail))" } else { '' }
    throw "Timeout waiting for $description.$suffix"
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
    $clockWidth = Convert-ToScaled 128
    $notificationWidth = Convert-ToScaled 30
    $controlWidth = Convert-ToScaled 30
    $right = $bar.Right - $padding
    $clockX = $right - $clockWidth
    $notificationX = $clockX - $gap - $notificationWidth
    $controlRight = $notificationX - $gap
    $controlX = $controlRight - $controlWidth
    return [pscustomobject]@{
        X = $controlX + [int]($controlWidth / 2)
        Y = $bar.Top + [int]($bar.Height / 2)
    }
}

function Get-StatusWifiPoint($bar) {
    $padding = Convert-ToScaled 8
    $gap = Convert-ToScaled 4
    $clockWidth = Convert-ToScaled 128
    $notificationWidth = Convert-ToScaled 30
    $controlWidth = Convert-ToScaled 30
    $searchWidth = Convert-ToScaled 30
    $statusWidth = Convert-ToScaled 108
    $iconUnit = [Math]::Max(1, (Convert-ToScaled 15))
    $clockX = $bar.Right - $padding - $clockWidth
    $notificationX = $clockX - $gap - $notificationWidth
    $controlX = $notificationX - $gap - $controlWidth
    $searchX = $controlX - $gap - $searchWidth
    $statusX = $searchX - $gap - $statusWidth
    $wifiWidth = [int][Math]::Floor(($iconUnit * 148 + 50) / 100)
    return [pscustomobject]@{
        X = $statusX + [int]($wifiWidth / 2)
        Y = $bar.Top + [int]($bar.Height / 2)
    }
}

function Get-ClockPoint($bar) {
    $padding = Convert-ToScaled 8
    $clockWidth = Convert-ToScaled 128
    $clockX = $bar.Right - $padding - $clockWidth
    return [pscustomobject]@{
        X = $clockX + [int]($clockWidth / 2)
        Y = $bar.Top + [int]($bar.Height / 2)
    }
}

function Get-NotificationPoint($bar) {
    $padding = Convert-ToScaled 8
    $gap = Convert-ToScaled 4
    $clockWidth = Convert-ToScaled 128
    $notificationWidth = Convert-ToScaled 30
    $clockX = $bar.Right - $padding - $clockWidth
    $notificationX = $clockX - $gap - $notificationWidth
    return [pscustomobject]@{
        X = $notificationX + [int]($notificationWidth / 2)
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
    $width = [Math]::Max(1, $script:screenWidth)
    $height = [Math]::Max(1, $script:screenHeight)
    $bitmap = [System.Drawing.Bitmap]::new($width, $height)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        try {
            $graphics.CopyFromScreen(0, 0, 0, 0, $bitmap.Size, [System.Drawing.CopyPixelOperation]::SourceCopy)
            $script:desktopCaptureAvailable = $true
        }
        catch {
            $script:desktopCaptureAvailable = $false
            $graphics.Clear([System.Drawing.Color]::FromArgb(255, 30, 30, 30))
        }
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
        try {
            $graphics.CopyFromScreen($window.Left, $window.Top, 0, 0, $bitmap.Size, [System.Drawing.CopyPixelOperation]::SourceCopy)
        }
        catch {
            return "fallback-$($window.ClassName)-$($window.Left),$($window.Top),$($window.Width),$($window.Height)"
        }
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
            if ($script:desktopCaptureAvailable -and $visualSignal -lt 5) {
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
    $controlCenter = Get-OttoWindow $process.Id 'OttoControlCenter' $true
    $controls = Get-OttoWindow $process.Id 'OttoWindowControls' $true
    if (-not $bar -or $bar.Width -lt 400 -or $bar.Height -lt 20 -or $bar.Top -gt 2) {
        throw 'Initial top bar geometry is not valid.'
    }
    if (-not $dock -or $dock.Width -lt 100 -or $dock.Height -lt 30) {
        throw 'Initial dock geometry is not valid.'
    }
    if ($panel -or $menu -or $controlCenter) {
        throw 'Initial popup, Control Center, or expanded panel is visible.'
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
    if (-not $menu -or $menu.Width -lt (Convert-ToScaled 210) -or $menu.Height -lt (Convert-ToScaled 80)) {
        throw 'Popup menu geometry is not valid.'
    }
    if ($menu.Left -lt 0 -or $menu.Top -lt 0 -or $menu.Right -gt $script:screenWidth -or $menu.Bottom -gt $script:screenHeight) {
        throw 'Popup menu is outside the primary screen bounds.'
    }
}

function Assert-ControlCenterTopology($process) {
    $cc = Get-OttoWindow $process.Id 'OttoControlCenter' $true
    if (-not $cc -or $cc.Width -lt (Convert-ToScaled 300) -or $cc.Height -lt (Convert-ToScaled 200)) {
        throw 'Control Center geometry is not valid.'
    }
    if ($cc.Left -lt 0 -or $cc.Top -lt 0 -or $cc.Right -gt $script:screenWidth -or $cc.Bottom -gt $script:screenHeight) {
        throw 'Control Center is outside the primary screen bounds.'
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
    Assert-NoCrash $process
}

function Assert-NoCrash($process) {
    $pidHex = '0x{0:X}' -f $process.Id
    $crash = Get-WinEvent -FilterHashtable @{ LogName = 'Application'; Id = 1000; StartTime = $process.StartTime } -ErrorAction SilentlyContinue |
        Where-Object { $_.Message -match 'otto-os\.exe' -and $_.Message -match "Faulting process id: $pidHex(?![0-9A-Fa-f])" } |
        Select-Object -First 1
    if ($crash) {
        $module = [regex]::Match($crash.Message, 'Faulting module name: ([^,]+)').Groups[1].Value
        throw "Otto process $($process.Id) crashed during its lifetime (module $module)."
    }
}

function Open-ControlCenter($process) {
    $bar = Get-BarMetrics $process
    $point = Get-ControlCenterPoint $bar
    [OttoE2ENative]::ClickWindow($bar.Handle, $point.X, $point.Y)
    Wait-VisibleWindow $process 'OttoControlCenter' 'Control Center popover' 3000
    Assert-ControlCenterTopology $process
}

function Close-ControlCenter($process) {
    [OttoE2ENative]::SendKey(0x1B)
    try {
        Wait-HiddenWindow $process 'OttoControlCenter' 'Control Center dismissal' 1000
        return
    }
    catch {
        $cc = Get-OttoWindow $process.Id 'OttoControlCenter' $true
        if ($cc) {
            [OttoE2ENative]::RequestClose($cc.Handle)
        } else {
            $bar = Get-BarMetrics $process
            [OttoE2ENative]::ClickAndHold($bar.Left + (Convert-ToScaled 80), $bar.Bottom + (Convert-ToScaled 80))
        }
        Wait-HiddenWindow $process 'OttoControlCenter' 'Control Center dismissal fallback' 3000
    }
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
        $popup = Get-OttoWindow $process.Id 'OttoPopupMenu' $true
        if ($popup) {
            [OttoE2ENative]::RequestClose($popup.Handle)
        } else {
            $bar = Get-BarMetrics $process
            [OttoE2ENative]::ClickAndHold($bar.Left + (Convert-ToScaled 80), $bar.Bottom + (Convert-ToScaled 80))
        }
        Wait-HiddenWindow $process 'OttoPopupMenu' 'popup dismissal fallback' 3000
    }
}

function Expand-Notch($process) {
    $bar = Get-BarMetrics $process
    [OttoE2ENative]::MoveCursor($bar.Left + (Convert-ToScaled 80), $bar.Bottom + (Convert-ToScaled 80))
    [OttoE2ENative]::SendCtrlAltKey(0x4E)
    try {
        Wait-VisibleWindow $process 'OttoNotchPanel' 'notch expansion' 2500
    }
    catch {
        $panel = Get-OttoWindow $process.Id 'OttoNotchPanel' $true
        if (-not $panel) {
            $barWin = Get-OttoWindow $process.Id 'OttoNotchBar' $true
            [OttoE2ENative]::PostHotkey($barWin.Handle, 1)
            Wait-VisibleWindow $process 'OttoNotchPanel' 'notch expansion fallback' 3000
        }
    }
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
    [OttoE2ENative]::SendCtrlAltKey(0x4E)
    try {
        Wait-HiddenWindow $process 'OttoNotchPanel' 'notch collapse' 3500
    }
    catch {
        $panel = Get-OttoWindow $process.Id 'OttoNotchPanel' $true
        if ($panel) {
            $fallbackScreenshot = Save-Screenshot $script:activeScenario 'notch-hotkey-warning'
            [void]$script:observations.Add([pscustomobject][ordered]@{
                scenario = $script:activeScenario
                checkpoint = 'notch-hotkey-input'
                expectation = 'Ctrl+Alt+N closes the expanded notch through the registered global hotkey.'
                status = 'WARNING'
                message = "Synthetic Ctrl+Alt+N close was not observed; WM_HOTKEY handler fallback used. screenshot=$fallbackScreenshot"
            })
            Write-Output 'OBSERVATION_WARNING ctrl-alt-n-close-input-not-observed'
            $barWin = Get-OttoWindow $process.Id 'OttoNotchBar' $true
            [OttoE2ENative]::PostHotkey($barWin.Handle, 1)
            Wait-HiddenWindow $process 'OttoNotchPanel' 'notch collapse handler fallback' 4000
        }
    }
}

function Run-SuccessFlow {
    $process = Start-Otto 'success'
    try {
        Assert-InitialTopology $process
        Record-Checkpoint $process '01-baseline' 'Top bar and dock are visible; popup and expanded panel are hidden.'
        Expand-Notch $process
        Record-Checkpoint $process '02-notch-expanded' 'Ctrl+Alt+N exposes a settled expanded notch panel within screen bounds.' 'OttoNotchPanel'
        Collapse-Notch $process
        Start-Sleep -Milliseconds 300
        Open-ControlCenter $process
        Record-Checkpoint $process '03-control-center-open' 'Control Center opens as an in-bounds Liquid Glass bento popover.' 'OttoControlCenter'
        Close-ControlCenter $process
        Start-Sleep -Milliseconds 200
        $bar = Get-BarMetrics $process
        $wifi = Get-StatusWifiPoint $bar
        [OttoE2ENative]::ClickWindow($bar.Handle, $wifi.X, $wifi.Y)
        Wait-VisibleWindow $process 'OttoControlCenter' 'Wi-Fi status deep-link' 3000
        Assert-ControlCenterTopology $process
        Record-Checkpoint $process '03b-control-center-wifi' 'Status Wi-Fi icon opens Control Center directly into the Wi-Fi view.' 'OttoControlCenter'
        Close-ControlCenter $process
        Start-Sleep -Milliseconds 200
        $bar = Get-BarMetrics $process
        $clock = Get-ClockPoint $bar
        [OttoE2ENative]::ClickWindow($bar.Handle, $clock.X, $clock.Y)
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
        Start-Sleep -Milliseconds 200
        $bar = Get-BarMetrics $process
        $notifications = Get-NotificationPoint $bar
        [OttoE2ENative]::ClickWindow($bar.Handle, $notifications.X, $notifications.Y)
        Wait-VisibleWindow $process 'OttoPopupMenu' 'notification popup' 3000
        Assert-MenuTopology $process
        Record-Checkpoint $process '04b-notifications-open' 'Bell click opens notifications independently from the calendar.' 'OttoPopupMenu'
        Close-Popup $process
        Start-Sleep -Milliseconds 200
        $bar = Get-BarMetrics $process
        $start = Get-StartPoint $bar
        [OttoE2ENative]::ClickWindow($bar.Handle, $start.X, $start.Y)
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
        if ((Get-OttoWindow $process.Id 'OttoPopupMenu' $true) -or (Get-OttoWindow $process.Id 'OttoControlCenter' $true)) {
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
        $panel = Get-OttoWindow $process.Id 'OttoNotchPanel' $true
        if ($panel) {
            [OttoE2ENative]::ClickWindow($panel.Handle, $media.X - $panel.Left, $media.Y - $panel.Top)
        } else {
            [OttoE2ENative]::ClickWindow($bar.Handle, $media.X, $media.Y)
        }
        Start-Sleep -Milliseconds 450
        Assert-ProcessAlive $process 'media tab without provider'
        Assert-ExpandedTopology $process
        Record-Checkpoint $process '01-empty-media-tab' 'Selecting Now Playing with no active provider keeps the expanded surface stable.' 'OttoNotchPanel'
        [OttoE2ENative]::SendCtrlAltKey(0x50)
        Start-Sleep -Milliseconds 300
        Assert-ProcessAlive $process 'pomodoro hotkey'
        Record-Checkpoint $process '02-pomodoro-hotkey' 'Ctrl+Alt+P changes local focus state without crashing or leaking a surface.' 'OttoNotchPanel'
        for ($index = 0; $index -lt 4; $index++) {
            [OttoE2ENative]::SendCtrlAltKey(0x4E)
        }
        Assert-ProcessAlive $process 'rapid notch toggles'
        Record-Checkpoint $process '03-rapid-notch-toggles' 'Repeated Ctrl+Alt+N input remains bounded and leaves the process responsive.'
        Start-Sleep -Milliseconds 600
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
        Record-Checkpoint $process '01-loaded-state' 'Panel and Control Center are both active before teardown stress.' 'OttoControlCenter'
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

function Get-ShellState {
    [pscustomobject]@{
        WorkArea = [OttoE2EShell]::WorkArea()
        TaskbarAutoHide = [OttoE2EShell]::TaskbarAutoHide()
        TaskbarVisible = [OttoE2EShell]::TaskbarVisible()
    }
}

function Test-ShellState($expected) {
    $actual = Get-ShellState
    return $actual.WorkArea -eq $expected.WorkArea -and
        $actual.TaskbarAutoHide -eq $expected.TaskbarAutoHide -and
        $actual.TaskbarVisible -eq $expected.TaskbarVisible
}

function Test-WorkArea($expected) {
    return [OttoE2EShell]::WorkArea() -eq $expected.WorkArea
}

function Get-RecoveryFilePath {
    Join-Path $env:LOCALAPPDATA 'otto-os\shell_recovery.json'
}

function Stop-OttoAbruptly($process) {
    Stop-Process -Id $process.Id -Force
    $process.WaitForExit(3000) | Out-Null
    $script:ownedProcess = $null
}

function Invoke-RestoreShell {
    Start-Process -FilePath $executable -ArgumentList '--restore-shell' -WorkingDirectory $repoRoot -Wait -WindowStyle Hidden
}

function Run-RecoveryFlow {
    $script:activeScenario = 'recovery'
    $recoveryFile = Get-RecoveryFilePath
    if (Test-Path -LiteralPath $recoveryFile) {
        throw "A shell recovery file already exists at $recoveryFile. Run the executable with --restore-shell before this scenario."
    }
    $baseline = Get-ShellState
    try {
        $process = Start-Otto 'recovery'
        Wait-Until { -not (Test-WorkArea $baseline) } 'Otto to reserve the top edge through the shell' 4000
        Wait-Until { -not [OttoE2EShell]::TaskbarVisible() } 'Otto to hide the system taskbar' 4000
        Stop-OttoAbruptly $process
        if (-not (Test-Path -LiteralPath $recoveryFile)) {
            throw 'No shell recovery file remained after abrupt termination.'
        }
        Wait-Until { Test-WorkArea $baseline } 'the shell to release the reservation of the terminated process' 6000
        if (Test-ShellState $baseline) {
            throw 'Abrupt termination left the taskbar visible, so the crash precondition was not met.'
        }

        $process = Start-Otto 'recovery'
        Stop-Otto $process
        Wait-Until { Test-ShellState $baseline } 'graceful exit after a crash to restore the original shell state' 4000
        if (Test-Path -LiteralPath $recoveryFile) {
            throw 'Recovery file was not cleared after graceful exit.'
        }

        $process = Start-Otto 'recovery'
        Wait-Until { -not (Test-WorkArea $baseline) } 'Otto to reserve the top edge again' 4000
        Stop-OttoAbruptly $process
        Wait-Until { Test-WorkArea $baseline } 'the shell to release the second reservation' 6000
        Invoke-RestoreShell
        Wait-Until { Test-ShellState $baseline } '--restore-shell to restore the original shell state' 4000
        if (Test-Path -LiteralPath $recoveryFile) {
            throw '--restore-shell did not clear the recovery file.'
        }
        Write-Output 'RECOVERY_FLOW_PASS the shell reclaimed the reservation after a forced kill; relaunch, graceful exit, and --restore-shell returned the taskbar to baseline.'
    }
    finally {
        if ($script:ownedProcess) { Stop-Otto $script:ownedProcess }
        if (Test-Path -LiteralPath $recoveryFile) { Invoke-RestoreShell }
    }
}

New-Item -ItemType Directory -Path $artifactRoot -Force | Out-Null
Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class OttoE2EWindowState
{
    [DllImport("user32.dll")]
    public static extern bool IsIconic(IntPtr window);

    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr window);

    [DllImport("user32.dll")]
    public static extern bool IsZoomed(IntPtr window);

    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr window, int command);

    [DllImport("user32.dll")]
    private static extern IntPtr GetForegroundWindow();

    public static IntPtr Foreground() { return GetForegroundWindow(); }
}
'@

function Start-ProbeWindow([string]$title) {
    $formScript = Join-Path $artifactRoot 'genie-probe.ps1'
    @"
Add-Type -Namespace OttoProbe -Name Console -MemberDefinition '[DllImport("kernel32.dll")] public static extern System.IntPtr GetConsoleWindow(); [DllImport("user32.dll")] public static extern bool ShowWindow(System.IntPtr window, int command);'
[void][OttoProbe.Console]::ShowWindow([OttoProbe.Console]::GetConsoleWindow(), 0)
Add-Type -AssemblyName System.Windows.Forms
`$form = New-Object Windows.Forms.Form
`$form.Text = '$title'
`$form.StartPosition = 'Manual'
`$form.Bounds = New-Object Drawing.Rectangle 240, 220, 720, 480
`$form.BackColor = 'White'
`$form.TopMost = `$true
[Windows.Forms.Application]::Run(`$form)
"@ | Set-Content -LiteralPath $formScript -Encoding utf8
    Start-Process powershell -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $formScript) -PassThru -WindowStyle Hidden
}

function Run-GenieFlow {
    $process = Start-Otto 'genie'
    $probe = Start-ProbeWindow "OttoE2EGenie$runId"
    try {
        $form = $null
        Wait-Until {
            $script:genieForm = @(Get-OttoWindows $probe.Id | Where-Object { $_.Visible -and $_.ClassName -like 'WindowsForms*' }) | Select-Object -First 1
            $null -ne $script:genieForm
        } 'probe window to open' 10000
        $form = $script:genieForm
        $minimized = Invoke-GenieMinimize $process $form 'genie'
        $dockIcon = $minimized.Icon
        $frames = $minimized.Frames
        if ($frames -lt 3) {
            throw "The Genie minimize overlay was observed in only $frames polls; the animation did not run."
        }
        Write-Output "CHECKPOINT_PASS scenario=genie name=01-genie-in-flight overlay_polls=$frames"
        Wait-Until { [OttoE2EWindowState]::IsIconic($form.Handle) } 'probe window to end minimized' 3000
        Wait-HiddenWindow $process 'OttoGenie' 'Genie overlay to hide after the animation' 3000
        Record-Checkpoint $process '02-minimized' 'Window is minimized and no overlay remains.'
        $ledger = Join-Path $env:LOCALAPPDATA 'otto-os\hidden_windows.txt'
        if (Test-Path -LiteralPath $ledger) {
            throw 'A window override was left in the ledger after minimizing.'
        }
        if (-not $dockIcon) {
            throw 'The final Genie frame was not observed, so the Dock icon position is unknown.'
        }
        Start-Sleep -Milliseconds 600
        $dockWindow = Get-OttoWindow $process.Id 'OttoDockBar' $true
        $iconCenterX = [int](($dockIcon.Left + $dockIcon.Right) / 2)
        $iconCenterY = $dockWindow.Bottom - (Convert-ToScaled 6) - (Convert-ToScaled 12) - (Convert-ToScaled 24)
        $restoreStarted = $false
        for ($attempt = 0; $attempt -lt 3 -and -not $restoreStarted; $attempt++) {
            [OttoE2ENative]::MoveCursor($iconCenterX, $dockWindow.Bottom - 1)
            Start-Sleep -Milliseconds 700
            [OttoE2ENative]::MoveCursor($iconCenterX, $iconCenterY)
            Start-Sleep -Milliseconds 400
            [OttoE2ENative]::Click($iconCenterX, $iconCenterY)
            $deadline = (Get-Date).AddMilliseconds(1500)
            while ((Get-Date) -lt $deadline) {
                if (Get-OttoWindow $process.Id 'OttoGenie' $true) {
                    $restoreStarted = $true
                    break
                }
                if (-not [OttoE2EWindowState]::IsIconic($form.Handle)) {
                    throw 'The probe window was restored without the Genie animation.'
                }
                Start-Sleep -Milliseconds 10
            }
        }
        if (-not $restoreStarted) {
            throw "Clicking the Dock icon never started a restore (clicked=$iconCenterX,$iconCenterY dock=$($dockWindow.Left),$($dockWindow.Top),$($dockWindow.Right),$($dockWindow.Bottom))."
        }
        Record-Checkpoint $process '03-restore-in-flight' 'Genie overlay grows from the Dock back to the window frame.' 'OttoGenie'
        Wait-Until { -not [OttoE2EWindowState]::IsIconic($form.Handle) -and [OttoE2EWindowState]::IsWindowVisible($form.Handle) } 'probe window to be restored' 3000
        Wait-HiddenWindow $process 'OttoGenie' 'Genie overlay to hide after restoring' 3000
        Record-Checkpoint $process '04-restored' 'Window is restored in place and no overlay remains.'
        if (Test-Path -LiteralPath $ledger) {
            throw 'A window override was left in the ledger after restoring.'
        }
    }
    finally {
        Stop-Process -Id $probe.Id -Force -ErrorAction SilentlyContinue
    }
    Stop-Otto $process
}

function Get-NativeMinimizeCenter($window) {
    if (-not ('OttoE2EDwm' -as [type])) {
        Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class OttoE2EDwm {
    [StructLayout(LayoutKind.Sequential)] public struct Bounds { public int Left, Top, Right, Bottom; }
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr window, int attribute, out Bounds bounds, int size);
}
"@
    }
    $frame = New-Object OttoE2EDwm+Bounds
    $buttons = New-Object OttoE2EDwm+Bounds
    if ([OttoE2EDwm]::DwmGetWindowAttribute($window.Handle, 9, [ref]$frame, 16) -ne 0 -or
        [OttoE2EDwm]::DwmGetWindowAttribute($window.Handle, 5, [ref]$buttons, 16) -ne 0) {
        throw 'DWM did not report the probe window caption buttons.'
    }
    return [pscustomobject]@{
        X = $frame.Right - (Convert-ToScaled 138) + (Convert-ToScaled 23)
        Y = $frame.Top + (Convert-ToScaled 16)
    }
}

function Run-NativeMinimizeFlow {
    $process = Start-Otto 'native-minimize'
    $probe = Start-ProbeWindow "OttoE2ENative$runId"
    try {
        $form = Wait-ProbeForm $probe 'native minimize probe window to open'
        $center = @([int](($form.Left + $form.Right) / 2), [int](($form.Top + $form.Bottom) / 2))
        [OttoE2ENative]::Click($center[0], $center[1])
        Wait-Until { [OttoE2EWindowState]::Foreground() -eq $form.Handle } 'probe window to take focus' 3000
        $button = Get-NativeMinimizeCenter $form
        [OttoE2ENative]::MoveCursor($button.X, $button.Y)
        Start-Sleep -Milliseconds 300
        Save-Screenshot 'native-minimize' '00-before-minimize' | Out-Null
        [OttoE2ENative]::Click($button.X, $button.Y)
        $overlaySeen = $false
        $deadline = (Get-Date).AddMilliseconds(800)
        while ((Get-Date) -lt $deadline) {
            if (Get-OttoWindow $process.Id 'OttoGenie' $true) {
                $overlaySeen = $true
                break
            }
            Start-Sleep -Milliseconds 10
        }
        if (-not $overlaySeen) {
            throw "The Genie overlay never appeared after the native minimize button was pressed at $($button.X),$($button.Y) (iconic=$([OttoE2EWindowState]::IsIconic($form.Handle)))."
        }
        Write-Output 'CHECKPOINT_PASS scenario=native-minimize name=01-genie-from-native-button'
        Wait-Until { [OttoE2EWindowState]::IsIconic($form.Handle) } 'probe window to end minimized' 3000
        Wait-HiddenWindow $process 'OttoGenie' 'Genie overlay to hide after the animation' 3000
        Write-Output 'CHECKPOINT_PASS scenario=native-minimize name=02-minimized'
    }
    finally {
        Stop-Process -Id $probe.Id -Force -ErrorAction SilentlyContinue
    }
    Stop-Otto $process
}

function Get-CaptionFrameRight($window) {
    if (-not ('OttoE2EDwm' -as [type])) {
        Get-NativeMinimizeCenter $window | Out-Null
    }
    $frame = New-Object OttoE2EDwm+Bounds
    if ([OttoE2EDwm]::DwmGetWindowAttribute($window.Handle, 9, [ref]$frame, 16) -ne 0) {
        throw 'DWM did not report the probe window frame.'
    }
    return $frame
}

function Get-CoverCircleCenterX($cover, [int]$slot) {
    $rightMargin = Convert-ToScaled 14
    $spacing = Convert-ToScaled 20
    $closeX = $cover.Right - $rightMargin
    return [int]($closeX - (2 - $slot) * $spacing)
}

function Assert-CoverCircle([string]$path, $cover, [int]$slot, [string]$name, [scriptblock]$isColor) {
    $x = Get-CoverCircleCenterX $cover $slot
    $y = [int](($cover.Top + $cover.Bottom) / 2)
    $color = Get-RegionColor $path ([pscustomobject]@{ X = $x - 2; Y = $y - 2; Width = 4; Height = 4 })
    if (-not (& $isColor $color)) {
        throw "The $name circle is not drawn at the expected traffic light position (rgb=$($color.R),$($color.G),$($color.B) at $x,$y)."
    }
}

function Run-WindowCoverFlow {
    $process = Start-Otto 'window-cover'
    $probe = Start-ProbeWindow "OttoE2ECover$runId"
    try {
        $form = Wait-ProbeForm $probe 'cover probe window to open'
        [OttoE2ENative]::Click([int](($form.Left + $form.Right) / 2), [int](($form.Top + $form.Bottom) / 2))
        Wait-Until { [OttoE2EWindowState]::Foreground() -eq $form.Handle } 'probe window to take focus' 3000
        Wait-Until { $null -ne (Get-OttoWindow $process.Id 'OttoWindowControls' $true) } 'cover overlay to appear' 5000
        Start-Sleep -Milliseconds 400
        $cover = Get-OttoWindow $process.Id 'OttoWindowControls' $true
        $frame = Get-CaptionFrameRight $form
        $expectedWidth = Convert-ToScaled 138
        if ([Math]::Abs($cover.Right - $frame.Right) -gt 2) {
            throw "The cover right edge $($cover.Right) does not match the window frame right edge $($frame.Right)."
        }
        if ([Math]::Abs($cover.Width - $expectedWidth) -gt 3) {
            throw "The cover is $($cover.Width) px wide; the native button strip is $expectedWidth px."
        }
        if ($cover.Top -lt $frame.Top -or $cover.Top -gt $frame.Top + (Convert-ToScaled 4)) {
            throw "The cover top $($cover.Top) is not aligned with the window top $($frame.Top)."
        }
        [OttoE2ENative]::MoveCursor($form.Left + 40, $form.Top + 200)
        Start-Sleep -Milliseconds 300
        $shot = Save-Screenshot 'window-cover' '01-cover'
        Assert-CoverCircle $shot $cover 0 'yellow minimize' { param($c) $c.R -gt 200 -and $c.G -gt 150 -and $c.B -lt 110 }
        Assert-CoverCircle $shot $cover 1 'green zoom' { param($c) $c.G -gt 150 -and $c.R -lt 120 }
        Assert-CoverCircle $shot $cover 2 'red close' { param($c) $c.R -gt 180 -and $c.G -lt 120 }
        $midY = [int](($cover.Top + $cover.Bottom) / 2)
        $titleBar = Get-RegionColor $shot ([pscustomobject]@{ X = $cover.Left - 12; Y = $midY - 2; Width = 4; Height = 4 })
        $coverFill = Get-RegionColor $shot ([pscustomobject]@{ X = $cover.Left + 2; Y = $midY - 2; Width = 4; Height = 4 })
        if (-not (Test-ColorNear $coverFill @($titleBar.R, $titleBar.G, $titleBar.B) 14)) {
            throw "The cover fill rgb=$($coverFill.R),$($coverFill.G),$($coverFill.B) does not match the title bar rgb=$($titleBar.R),$($titleBar.G),$($titleBar.B)."
        }
        Write-Output 'CHECKPOINT_PASS scenario=window-cover name=01-covers-native-buttons'
        $y = [int](($cover.Top + $cover.Bottom) / 2)
        $zoomX = Get-CoverCircleCenterX $cover 1
        [OttoE2ENative]::Click($zoomX, $y)
        Wait-Until { [OttoE2EWindowState]::IsZoomed($form.Handle) } 'green button to maximize the probe window' 3000
        Start-Sleep -Milliseconds 500
        Save-Screenshot 'window-cover' '02-maximized' | Out-Null
        Write-Output 'CHECKPOINT_PASS scenario=window-cover name=02-zoom-maximizes'
        [void][OttoE2EWindowState]::ShowWindow($form.Handle, 9)
        Wait-Until { -not [OttoE2EWindowState]::IsZoomed($form.Handle) } 'probe window to restore' 3000
        Wait-Until { $null -ne (Get-OttoWindow $process.Id 'OttoWindowControls' $true) } 'cover overlay to return after restore' 5000
        Start-Sleep -Milliseconds 500
        $cover = Get-OttoWindow $process.Id 'OttoWindowControls' $true
        $y = [int](($cover.Top + $cover.Bottom) / 2)
        $closeX = Get-CoverCircleCenterX $cover 2
        [OttoE2ENative]::Click($closeX, $y)
        Wait-Until { -not [OttoE2EWindowState]::IsWindowVisible($form.Handle) } 'red button to close the probe window' 3000
        Write-Output 'CHECKPOINT_PASS scenario=window-cover name=03-close-closes'
        Start-Sleep -Milliseconds 800
        Assert-NoCrash $process
        Write-Output 'CHECKPOINT_PASS scenario=window-cover name=04-survives-close'
    }
    finally {
        Stop-Process -Id $probe.Id -Force -ErrorAction SilentlyContinue
    }
    Stop-Otto $process
}

function Start-FullScreenWindow([string]$title) {
    $formScript = Join-Path $artifactRoot 'fullscreen-probe.ps1'
    @"
Add-Type -Namespace OttoProbe -Name Console -MemberDefinition '[DllImport("kernel32.dll")] public static extern System.IntPtr GetConsoleWindow(); [DllImport("user32.dll")] public static extern bool ShowWindow(System.IntPtr window, int command);'
[void][OttoProbe.Console]::ShowWindow([OttoProbe.Console]::GetConsoleWindow(), 0)
Add-Type -AssemblyName System.Windows.Forms
`$form = New-Object Windows.Forms.Form
`$form.Text = '$title'
`$form.FormBorderStyle = 'None'
`$form.StartPosition = 'Manual'
`$form.Bounds = [Windows.Forms.Screen]::PrimaryScreen.Bounds
`$form.BackColor = 'Black'
`$form.TopMost = `$true
[Windows.Forms.Application]::Run(`$form)
"@ | Set-Content -LiteralPath $formScript -Encoding utf8
    Start-Process powershell -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $formScript) -PassThru -WindowStyle Hidden
}

function Run-FullScreenFlow {
    $process = Start-Otto 'fullscreen'
    $title = "OttoE2EFullScreen$runId"
    $probe = Start-FullScreenWindow $title
    try {
        Wait-Until {
            $script:fullScreenForm = @(Get-OttoWindows $probe.Id | Where-Object { $_.Visible -and $_.ClassName -like 'WindowsForms*' }) | Select-Object -First 1
            $null -ne $script:fullScreenForm
        } 'full-screen probe window to open' 10000
        $probeWindow = $script:fullScreenForm
        [OttoE2ENative]::Click([int](($probeWindow.Left + $probeWindow.Right) / 2), [int](($probeWindow.Top + $probeWindow.Bottom) / 2))
        Wait-HiddenWindow $process 'OttoNotchBar' 'top bar to hide behind a full-screen window' 8000
        Record-Checkpoint $process '01-hidden-for-full-screen' 'Top bar is hidden while a monitor-sized window is in the foreground.'
    }
    finally {
        Stop-Process -Id $probe.Id -Force -ErrorAction SilentlyContinue
    }
    Wait-VisibleWindow $process 'OttoNotchBar' 'top bar to return after the full-screen window closes' 8000
    Record-Checkpoint $process '02-visible-after-full-screen' 'Top bar returns once the full-screen window is gone.' 'OttoNotchBar'
    Stop-Otto $process
}

function Get-RegionDiff([string]$beforePath, [string]$afterPath, $region) {
    $before = [System.Drawing.Bitmap]::new($beforePath)
    $after = [System.Drawing.Bitmap]::new($afterPath)
    try {
        $changed = 0
        $right = [Math]::Min($before.Width, $region.X + $region.Width)
        $bottom = [Math]::Min($before.Height, $region.Y + $region.Height)
        for ($y = [Math]::Max(0, $region.Y); $y -lt $bottom; $y++) {
            for ($x = [Math]::Max(0, $region.X); $x -lt $right; $x++) {
                $a = $before.GetPixel($x, $y)
                $b = $after.GetPixel($x, $y)
                if ([Math]::Abs($a.R - $b.R) -gt 40 -or [Math]::Abs($a.G - $b.G) -gt 40 -or [Math]::Abs($a.B - $b.B) -gt 40) {
                    $changed++
                }
            }
        }
        return $changed
    }
    finally {
        $before.Dispose()
        $after.Dispose()
    }
}

function Get-RegionColor([string]$path, $region) {
    $bitmap = [System.Drawing.Bitmap]::new($path)
    try {
        $count = 0
        $r = 0.0; $g = 0.0; $b = 0.0
        $minLuma = 255.0; $maxLuma = 0.0
        for ($y = $region.Y; $y -lt ($region.Y + $region.Height); $y += 2) {
            for ($x = $region.X; $x -lt ($region.X + $region.Width); $x += 2) {
                $pixel = $bitmap.GetPixel($x, $y)
                $r += $pixel.R; $g += $pixel.G; $b += $pixel.B
                $luma = 0.2126 * $pixel.R + 0.7152 * $pixel.G + 0.0722 * $pixel.B
                $minLuma = [Math]::Min($minLuma, $luma)
                $maxLuma = [Math]::Max($maxLuma, $luma)
                $count++
            }
        }
        return [pscustomobject]@{
            R = [int]($r / $count)
            G = [int]($g / $count)
            B = [int]($b / $count)
            Spread = [int]($maxLuma - $minLuma)
        }
    }
    finally {
        $bitmap.Dispose()
    }
}

function Test-ColorNear($color, [int[]]$expected, [int]$tolerance) {
    return [Math]::Abs($color.R - $expected[0]) -le $tolerance -and
        [Math]::Abs($color.G - $expected[1]) -le $tolerance -and
        [Math]::Abs($color.B - $expected[2]) -le $tolerance
}

function Get-EmptyBarRegion($bar) {
    return [pscustomobject]@{
        X = $bar.Left + [int]($bar.Width * 0.30)
        Y = $bar.Top + (Convert-ToScaled 6)
        Width = [int]($bar.Width * 0.08)
        Height = Convert-ToScaled 20
    }
}

function Invoke-GenieMinimize($process, $form, [string]$scenario) {
    $center = @([int](($form.Left + $form.Right) / 2), [int](($form.Top + $form.Bottom) / 2))
    $tolerance = Convert-ToScaled 48
    $controls = $null
    for ($attempt = 0; $attempt -lt 3 -and -not $controls; $attempt++) {
        [OttoE2ENative]::Click($center[0], $center[1])
        $deadline = (Get-Date).AddMilliseconds(1500)
        while ((Get-Date) -lt $deadline) {
            $candidate = Get-OttoWindow $process.Id 'OttoWindowControls' $true
            $focused = [OttoE2EWindowState]::Foreground() -eq $form.Handle
            if ($focused -and $candidate -and
                (([Math]::Abs($candidate.Left - $form.Left) -le $tolerance) -or ([Math]::Abs($candidate.Right - $form.Right) -le $tolerance)) -and
                [Math]::Abs($candidate.Top - $form.Top) -le $tolerance) {
                $controls = $candidate
                break
            }
            Start-Sleep -Milliseconds 30
        }
    }
    if (-not $controls) {
        throw 'Window controls never attached to the focused probe window; refusing to press minimize.'
    }
    $coverMode = [Math]::Abs($controls.Right - $form.Right) -le $tolerance
    $pressX = if ($coverMode) { Get-CoverCircleCenterX $controls 0 } else { $controls.Left + (Convert-ToScaled 32) }
    $pressY = if ($coverMode) { [int](($controls.Top + $controls.Bottom) / 2) } else { $controls.Top + (Convert-ToScaled 14) }
    [OttoE2ENative]::MoveCursor($pressX, $pressY)
    Start-Sleep -Milliseconds 300
    $settled = Get-OttoWindow $process.Id 'OttoWindowControls' $true
    if (-not $settled -or [OttoE2EWindowState]::Foreground() -ne $form.Handle -or
        ([Math]::Abs($settled.Left - $form.Left) -gt $tolerance -and [Math]::Abs($settled.Right - $form.Right) -gt $tolerance) -or [Math]::Abs($settled.Top - $form.Top) -gt $tolerance) {
        throw 'Focus or window controls moved away from the probe before minimize; leave the desktop untouched while this scenario runs.'
    }
    Save-Screenshot $scenario '00-before-minimize' | Out-Null
    [OttoE2ENative]::Click($pressX, $pressY)
    $overlaySeen = $false
    $deadline = (Get-Date).AddMilliseconds(600)
    while ((Get-Date) -lt $deadline) {
        if (Get-OttoWindow $process.Id 'OttoGenie' $true) {
            $overlaySeen = $true
            break
        }
        Start-Sleep -Milliseconds 10
    }
    if (-not $overlaySeen) {
        Start-Sleep -Milliseconds 800
        throw "The Genie overlay never appeared after the minimize button was pressed. (iconic=$([OttoE2EWindowState]::IsIconic($form.Handle)) controls=$($controls.Left),$($controls.Top) probe=$($form.Left),$($form.Top))"
    }
    $dockIcon = $null
    $frames = 0
    $deadline = (Get-Date).AddMilliseconds(3000)
    while ((Get-Date) -lt $deadline) {
        $overlay = Get-OttoWindow $process.Id 'OttoGenie' $true
        if ($overlay) { $dockIcon = $overlay; $frames++ } elseif ($dockIcon) { break }
        Start-Sleep -Milliseconds 10
    }
    return [pscustomobject]@{ Icon = $dockIcon; Frames = $frames }
}

function Get-DockIconPoint($process, $icon) {
    $dockWindow = Get-OttoWindow $process.Id 'OttoDockBar' $true
    if (-not $dockWindow -or -not $icon) {
        throw 'The Dock window or the probe icon position is unknown.'
    }
    return [pscustomobject]@{
        X = [int](($icon.Left + $icon.Right) / 2)
        Y = $dockWindow.Bottom - (Convert-ToScaled 6) - (Convert-ToScaled 12) - (Convert-ToScaled 24)
        Edge = $dockWindow.Bottom - 1
    }
}

function Restore-FromDock($process, $form, $point, [string]$description) {
    for ($attempt = 0; $attempt -lt 3; $attempt++) {
        Show-DockAt $point
        [OttoE2ENative]::Click($point.X, $point.Y)
        $deadline = (Get-Date).AddMilliseconds(2000)
        while ((Get-Date) -lt $deadline) {
            if (-not [OttoE2EWindowState]::IsIconic($form.Handle) -and [OttoE2EWindowState]::IsWindowVisible($form.Handle)) {
                return
            }
            Start-Sleep -Milliseconds 30
        }
    }
    $dock = Get-OttoWindow $process.Id 'OttoDockBar' $true
    throw "Timeout waiting for $description (clicked=$($point.X),$($point.Y) dock=$($dock.Left),$($dock.Top),$($dock.Right),$($dock.Bottom) iconic=$([OttoE2EWindowState]::IsIconic($form.Handle)) visible=$([OttoE2EWindowState]::IsWindowVisible($form.Handle)))."
}

function Show-DockAt($point) {
    [OttoE2ENative]::MoveCursor($point.X, $point.Edge)
    Start-Sleep -Milliseconds 700
    [OttoE2ENative]::MoveCursor($point.X, $point.Y)
    Start-Sleep -Milliseconds 400
}

function Wait-ProbeForm($probe, [string]$description) {
    Wait-Until {
        $script:probeForm = @(Get-OttoWindows $probe.Id | Where-Object { $_.Visible -and $_.ClassName -like 'WindowsForms*' }) | Select-Object -First 1
        $null -ne $script:probeForm
    } $description 20000 {
        $probe.Refresh()
        $windows = @(Get-OttoWindows $probe.Id | ForEach-Object { "$($_.ClassName)/$($_.Visible)" }) -join ';'
        "pid=$($probe.Id) exited=$($probe.HasExited) windows=$windows"
    }
    return $script:probeForm
}

function Invoke-DockMenuCommand($process, $form, $point, [byte]$letter, [string]$checkpoint) {
    [OttoE2ESystem]::Focus($form.Handle)
    Start-Sleep -Milliseconds 200
    Show-DockAt $point
    [OttoE2ESystem]::RightClick($point.X, $point.Y)
    Wait-VisibleWindow $process 'OttoPopupMenu' "Dock context menu for $checkpoint" 3000
    Record-Checkpoint $process $checkpoint "Dock context menu is open before choosing '$([char]$letter)'." 'OttoPopupMenu'
    [OttoE2ESystem]::HoldKey($letter, 80)
    Start-Sleep -Milliseconds 150
    [OttoE2ESystem]::HoldKey(0x0D, 80)
    Wait-HiddenWindow $process 'OttoPopupMenu' "Dock context menu to close after $checkpoint" 3000
}

function Run-DockFlow {
    $process = Start-Otto 'dock'
    $pinnedPath = Join-Path $env:LOCALAPPDATA 'otto-os\pinned.txt'
    $pinnedBackup = if (Test-Path -LiteralPath $pinnedPath) { [IO.File]::ReadAllBytes($pinnedPath) } else { $null }
    $probe = Start-ProbeWindow "OttoE2EDock$runId"
    $pinProbe = $null
    try {
        $form = Wait-ProbeForm $probe 'dock probe window to open'
        $minimized = Invoke-GenieMinimize $process $form 'dock'
        Wait-Until { [OttoE2EWindowState]::IsIconic($form.Handle) } 'dock probe to minimize' 3000
        Wait-HiddenWindow $process 'OttoGenie' 'Genie overlay to hide after minimize' 3000
        Start-Sleep -Milliseconds 600
        $point = Get-DockIconPoint $process $minimized.Icon

        Restore-FromDock $process $form $point 'Dock click to restore the window (ShowWindow effect)'
        Wait-HiddenWindow $process 'OttoGenie' 'Genie overlay to hide after restore' 3000
        Record-Checkpoint $process '01-restored-from-dock' 'Clicking the Dock icon restored the minimized window.'

        Invoke-DockMenuCommand $process $form $point ([byte][char]'H') '02-menu-before-hide'
        Wait-Until { -not [OttoE2EWindowState]::IsWindowVisible($form.Handle) } 'Hide from the Dock menu to hide the application' 3000
        Record-Checkpoint $process '03-hidden' 'Hide removed the application window from screen.'

        Restore-FromDock $process $form $point 'Dock click to unhide the application (Unhide effect)'
        Record-Checkpoint $process '04-unhidden' 'Clicking the Dock icon showed the hidden application again.'
        $ledger = Join-Path $env:LOCALAPPDATA 'otto-os\hidden_windows.txt'
        if (Test-Path -LiteralPath $ledger) {
            throw 'The hidden-window ledger still lists a window after unhide.'
        }

        Invoke-DockMenuCommand $process $form $point ([byte][char]'Q') '05-menu-before-quit'
        Wait-Until { $probe.HasExited } 'Quit from the Dock menu to end the application (Quit effect)' 5000
        Write-Output 'CHECKPOINT_PASS scenario=dock name=06-quit probe_exited=true'

        $pinProbe = Start-ProbeWindow "OttoE2EPin$runId"
        $pinForm = Wait-ProbeForm $pinProbe 'pin probe window to open'
        $pinMinimized = Invoke-GenieMinimize $process $pinForm 'dock'
        Wait-Until { [OttoE2EWindowState]::IsIconic($pinForm.Handle) } 'pin probe to minimize' 3000
        Wait-HiddenWindow $process 'OttoGenie' 'Genie overlay to hide after the pin probe minimized' 3000
        Start-Sleep -Milliseconds 600
        $pinPoint = Get-DockIconPoint $process $pinMinimized.Icon
        $before = if (Test-Path -LiteralPath $pinnedPath) { Get-Content -LiteralPath $pinnedPath -Raw } else { '' }
        Invoke-DockMenuCommand $process $pinForm $pinPoint ([byte][char]'K') '07-menu-before-keep'
        Wait-Until {
            $now = if (Test-Path -LiteralPath $pinnedPath) { Get-Content -LiteralPath $pinnedPath -Raw } else { '' }
            $now -ne $before
        } 'Keep in Dock to update the pinned list (TogglePin effect)' 3000
        Write-Output 'CHECKPOINT_PASS scenario=dock name=08-pinned-list-updated'

        $pinned = Get-Content -LiteralPath $pinnedPath -Raw
        [OttoE2ESystem]::Restore($pinForm.Handle)
        Wait-Until { -not [OttoE2EWindowState]::IsIconic($pinForm.Handle) -and [OttoE2EWindowState]::IsWindowVisible($pinForm.Handle) } 'pinned probe to restore' 3000
        Start-Sleep -Milliseconds 400
        $pinnedMinimized = Invoke-GenieMinimize $process $pinForm 'dock'
        Wait-Until { [OttoE2EWindowState]::IsIconic($pinForm.Handle) } 'pinned probe to minimize again' 3000
        Wait-HiddenWindow $process 'OttoGenie' 'Genie overlay to hide after the pinned probe minimized' 3000
        Start-Sleep -Milliseconds 600
        $pinnedPoint = Get-DockIconPoint $process $pinnedMinimized.Icon
        $liftY = $pinnedPoint.Y - (Convert-ToScaled 160)

        Show-DockAt $pinnedPoint
        [OttoE2ESystem]::LeftDown($pinnedPoint.X, $pinnedPoint.Y)
        [OttoE2ESystem]::DragTo($pinnedPoint.X, $pinnedPoint.Y, $pinnedPoint.X, $liftY, 20)
        Start-Sleep -Milliseconds 200
        [OttoE2ESystem]::HoldKey(0x1B, 350)
        Start-Sleep -Milliseconds 200
        [OttoE2ESystem]::LeftUp()
        Start-Sleep -Milliseconds 1200
        if ((Get-Content -LiteralPath $pinnedPath -Raw) -ne $pinned) {
            throw 'Cancelling a Dock drag with Escape changed the pinned list.'
        }
        Write-Output 'CHECKPOINT_PASS scenario=dock name=09-cancelled-drag-keeps-pins'

        Show-DockAt $pinnedPoint
        [OttoE2ESystem]::LeftDown($pinnedPoint.X, $pinnedPoint.Y)
        [OttoE2ESystem]::DragTo($pinnedPoint.X, $pinnedPoint.Y, $pinnedPoint.X, $liftY, 20)
        Start-Sleep -Milliseconds 200
        [OttoE2ESystem]::LeftUp()
        Wait-Until { (Get-Content -LiteralPath $pinnedPath -Raw) -ne $pinned } 'dragging a pinned icon out of the Dock to unpin it' 3000
        Write-Output 'CHECKPOINT_PASS scenario=dock name=10-drag-out-unpins'
    }
    finally {
        Stop-Process -Id $probe.Id -Force -ErrorAction SilentlyContinue
        if ($pinProbe) { Stop-Process -Id $pinProbe.Id -Force -ErrorAction SilentlyContinue }
        if ($null -ne $pinnedBackup) {
            [IO.File]::WriteAllBytes($pinnedPath, $pinnedBackup)
        }
        elseif (Test-Path -LiteralPath $pinnedPath) {
            Remove-Item -LiteralPath $pinnedPath -Force
        }
    }
    Stop-Otto $process
}

function Run-KeyboardFlow {
    Assert-DesktopHasFocus
    $process = Start-Otto 'keyboard'
    $bar = Get-BarMetrics $process
    $region = [pscustomobject]@{
        X = $bar.Left
        Y = $bar.Top
        Width = Convert-ToScaled 56
        Height = $bar.Height
    }
    Start-Sleep -Milliseconds 600
    $baseline = Save-Screenshot 'keyboard' '00-baseline'
    [OttoE2ESystem]::Chord(0x11, 0x71)
    Start-Sleep -Milliseconds 400
    $focused = Save-Screenshot 'keyboard' '01-focus-ring'
    $ring = Get-RegionDiff $baseline $focused $region
    if ($ring -lt 40) {
        throw "Ctrl+F2 did not draw a focus ring on the first bar slot (changed pixels: $ring)."
    }
    Write-Output "CHECKPOINT_PASS scenario=keyboard name=01-focus-ring changed=$ring"

    Start-Sleep -Milliseconds 16500
    $expired = Save-Screenshot 'keyboard' '02-idle-expired'
    $left = Get-RegionDiff $baseline $expired $region
    if ($left -gt 10) {
        throw "Focus mode did not expire after 15 s without navigation keys (changed pixels: $left)."
    }
    Write-Output "CHECKPOINT_PASS scenario=keyboard name=02-idle-expired changed=$left"

    [OttoE2ESystem]::Chord(0x11, 0x71)
    Start-Sleep -Milliseconds 400
    $again = Save-Screenshot 'keyboard' '03-focus-again'
    if ((Get-RegionDiff $baseline $again $region) -lt 40) {
        throw 'Ctrl+F2 did not re-enter focus mode after it expired.'
    }
    $empty = Get-EmptyBarRegion $bar
    [OttoE2ENative]::Click($empty.X + [int]($empty.Width / 2), $bar.Top + [int]($bar.Height / 2))
    Start-Sleep -Milliseconds 400
    $clicked = Save-Screenshot 'keyboard' '04-cleared-by-click'
    $residue = Get-RegionDiff $baseline $clicked $region
    if ($residue -gt 10) {
        throw "A mouse click on the bar did not clear the focus ring (changed pixels: $residue)."
    }
    Write-Output "CHECKPOINT_PASS scenario=keyboard name=04-cleared-by-click changed=$residue"
    Stop-Otto $process
}

function Assert-DesktopHasFocus {
    if (-not ('OttoE2EFocusProbe' -as [type])) {
        Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class OttoE2EFocusProbe {
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
}
"@
    }
    $handle = [OttoE2EFocusProbe]::GetForegroundWindow()
    $processId = [uint32]0
    [void][OttoE2EFocusProbe]::GetWindowThreadProcessId($handle, [ref]$processId)
    $name = (Get-Process -Id $processId -ErrorAction SilentlyContinue).ProcessName
    if ($name -notin @('explorer', 'otto-os')) {
        throw "Keyboard scenarios need the desktop in front, but '$name' has focus. Minimize it and rerun; the keys would otherwise reach that app."
    }
}

function Post-OttoMessage([IntPtr]$window, [uint32]$message, [IntPtr]$argument) {
    if (-not ('OttoE2EPost' -as [type])) {
        Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class OttoE2EPost {
    [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);
}
"@
    }
    if (-not [OttoE2EPost]::PostMessageW($window, $message, $argument, [IntPtr]::Zero)) {
        throw "PostMessage 0x$($message.ToString('X')) to the window failed."
    }
}

function Run-KeyboardDockFlow {
    $process = Start-Otto 'keyboard-dock'
    $bar = Get-BarMetrics $process
    $dock = Get-OttoWindow $process.Id 'OttoDockBar' $true
    if (-not $dock) {
        throw 'OttoDockBar is not visible.'
    }
    $barRegion = [pscustomobject]@{
        X = $bar.Left
        Y = $bar.Top
        Width = Convert-ToScaled 56
        Height = $bar.Height
    }
    $dockRegion = [pscustomobject]@{
        X = $dock.Left
        Y = $dock.Top
        Width = $dock.Width
        Height = $dock.Height
    }
    Start-Sleep -Milliseconds 600
    $barBaseline = Save-Screenshot 'keyboard-dock' '00-bar-baseline'
    $dockBaseline = Save-Screenshot 'keyboard-dock' '00-dock-baseline'

    Post-OttoMessage $bar.Handle 0x0312 ([IntPtr]3)
    Start-Sleep -Milliseconds 400
    Post-OttoMessage $bar.Handle 0x8015 ([IntPtr]3)
    Start-Sleep -Milliseconds 300
    Post-OttoMessage $bar.Handle 0x8015 ([IntPtr]7)
    Start-Sleep -Milliseconds 400
    $handed = Save-Screenshot 'keyboard-dock' '01-dock-focus-ring'
    $dockRing = Get-RegionDiff $dockBaseline $handed $dockRegion
    if ($dockRing -lt 40) {
        throw "Tab from the last bar slot did not draw a focus ring on the Dock (changed pixels: $dockRing)."
    }
    Write-Output "CHECKPOINT_PASS scenario=keyboard-dock name=01-dock-focus-ring changed=$dockRing"

    $barLeft = Get-RegionDiff $barBaseline $handed $barRegion
    if ($barLeft -gt 10) {
        throw "Bar focus ring stayed on after focus moved to the Dock (changed pixels: $barLeft)."
    }
    Write-Output "CHECKPOINT_PASS scenario=keyboard-dock name=02-bar-released changed=$barLeft"

    Post-OttoMessage $dock.Handle 0x8017 ([IntPtr]5)
    Start-Sleep -Milliseconds 400
    $exited = Save-Screenshot 'keyboard-dock' '03-dock-exited'
    $dockResidue = Get-RegionDiff $dockBaseline $exited $dockRegion
    if ($dockResidue -gt 10) {
        throw "Escape did not clear the Dock focus ring (changed pixels: $dockResidue)."
    }
    Write-Output "CHECKPOINT_PASS scenario=keyboard-dock name=03-dock-exited changed=$dockResidue"
    Stop-Otto $process
}

function Start-TypingProbe([string]$title) {
    $formScript = Join-Path $artifactRoot 'typing-probe.ps1'
    @"
Add-Type -Namespace OttoProbe -Name Console -MemberDefinition '[DllImport("kernel32.dll")] public static extern System.IntPtr GetConsoleWindow(); [DllImport("user32.dll")] public static extern bool ShowWindow(System.IntPtr window, int command);'
[void][OttoProbe.Console]::ShowWindow([OttoProbe.Console]::GetConsoleWindow(), 0)
Add-Type -AssemblyName System.Windows.Forms
`$form = New-Object Windows.Forms.Form
`$form.Text = '$title'
`$form.StartPosition = 'Manual'
`$form.Bounds = New-Object Drawing.Rectangle 240, 220, 520, 200
`$form.TopMost = `$true
`$box = New-Object Windows.Forms.TextBox
`$box.Dock = 'Fill'
`$box.Multiline = `$true
`$form.Controls.Add(`$box)
[Windows.Forms.Application]::Run(`$form)
"@ | Set-Content -LiteralPath $formScript -Encoding utf8
    Start-Process powershell -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $formScript) -PassThru -WindowStyle Hidden
}

function Run-StallFlow {
    $process = Start-Otto 'stall'
    $probe = Start-TypingProbe "OttoE2EStall$runId"
    $thread = [IntPtr]::Zero
    try {
        $form = Wait-ProbeForm $probe 'typing probe window to open'
        $box = [IntPtr]::Zero
        Wait-Until {
            $script:typingBox = [OttoE2ESystem]::FindChildByClassPrefix($form.Handle, 'WindowsForms10.EDIT')
            $script:typingBox -ne [IntPtr]::Zero
        } 'typing probe text box' 5000
        $box = $script:typingBox
        Wait-Until {
            [OttoE2ENative]::Click([int](($form.Left + $form.Right) / 2), [int](($form.Top + $form.Bottom) / 2))
            Start-Sleep -Milliseconds 150
            if ([OttoE2EWindowState]::Foreground() -ne $form.Handle) {
                [OttoE2ESystem]::Focus($form.Handle)
                Start-Sleep -Milliseconds 150
            }
            [OttoE2EWindowState]::Foreground() -eq $form.Handle
        } 'typing probe to take focus' 4000 {
            "foreground=$([OttoE2EWindowState]::Foreground()) form=$($form.Handle) bounds=$($form.Left),$($form.Top),$($form.Right),$($form.Bottom)"
        }
        $bar = Get-OttoWindow $process.Id 'OttoNotchBar' $true
        $thread = [OttoE2ESystem]::SuspendWindowThread($bar.Handle)
        if ($thread -eq [IntPtr]::Zero) {
            throw 'Could not suspend the Otto UI thread.'
        }
        Start-Sleep -Milliseconds 300
        $timer = [Diagnostics.Stopwatch]::StartNew()
        [OttoE2ESystem]::Type('ottostall')
        Wait-Until { [OttoE2ESystem]::WindowText($box) -eq 'ottostall' } 'typed text to reach another application while the Otto UI thread is stalled' 1500 {
            "text='$([OttoE2ESystem]::WindowText($box))'"
        }
        $latency = $timer.ElapsedMilliseconds
        Start-Sleep -Milliseconds 1200
        [OttoE2ESystem]::ResumeAndClose($thread)
        $thread = [IntPtr]::Zero
        if ($latency -gt 500) {
            throw "Typing reached the other application only after $latency ms while the Otto UI thread was stalled."
        }
        Write-Output "CHECKPOINT_PASS scenario=stall name=01-typing-during-ui-stall latency_ms=$latency"
        Start-Sleep -Milliseconds 500
        Assert-ProcessAlive $process 'UI thread resume'
        Record-Checkpoint $process '02-resumed' 'Otto keeps running after its UI thread resumes.' 'OttoNotchBar'
    }
    finally {
        [OttoE2ESystem]::ResumeAndClose($thread)
        Stop-Process -Id $probe.Id -Force -ErrorAction SilentlyContinue
    }
    Stop-Otto $process
}

function Set-PersonalizeValue([string]$name, $value) {
    $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize'
    if ($null -eq $value) {
        Remove-ItemProperty -Path $key -Name $name -ErrorAction SilentlyContinue
    }
    else {
        Set-ItemProperty -Path $key -Name $name -Value ([int]$value) -Type DWord
    }
}

function Find-DockPill([string]$path, $dock, [int[]]$hint, [int]$tolerance) {
    return Find-FillBox $path $dock.Left $dock.Top $dock.Width $dock.Height $hint $tolerance 'Dock pill'
}

function Find-FillBox([string]$path, [int]$left, [int]$top, [int]$width, [int]$height, [int[]]$hint, [int]$tolerance, [string]$label) {
    $bitmap = [System.Drawing.Bitmap]::new($path)
    try {
        $bottom = [Math]::Min($bitmap.Height - 1, $top + $height - 1)
        $right = [Math]::Min($bitmap.Width - 1, $left + $width - 1)
        $rowCount = @{}
        $rowMin = @{}
        $rowMax = @{}
        for ($y = [Math]::Max(0, $top); $y -le $bottom; $y += 2) {
            for ($x = [Math]::Max(0, $left); $x -le $right; $x += 2) {
                $pixel = $bitmap.GetPixel($x, $y)
                if ([Math]::Abs($pixel.R - $hint[0]) -le $tolerance -and
                    [Math]::Abs($pixel.G - $hint[1]) -le $tolerance -and
                    [Math]::Abs($pixel.B - $hint[2]) -le $tolerance) {
                    $rowCount[$y] = 1 + [int]$rowCount[$y]
                    if (-not $rowMin.ContainsKey($y) -or $x -lt $rowMin[$y]) { $rowMin[$y] = $x }
                    if (-not $rowMax.ContainsKey($y) -or $x -gt $rowMax[$y]) { $rowMax[$y] = $x }
                }
            }
        }
    }
    finally {
        $bitmap.Dispose()
    }
    if ($rowCount.Count -eq 0) {
        throw "$label not found: no pixel matched the fill hint ($($hint -join ','), tolerance $tolerance)."
    }
    $peak = ($rowCount.Values | Measure-Object -Maximum).Maximum
    $solid = @($rowCount.Keys | Where-Object { $rowCount[$_] -ge [Math]::Max(8, [int]($peak * 0.5)) })
    $minX = ($solid | ForEach-Object { $rowMin[$_] } | Measure-Object -Minimum).Minimum
    $maxX = ($solid | ForEach-Object { $rowMax[$_] } | Measure-Object -Maximum).Maximum
    $minY = ($solid | Measure-Object -Minimum).Minimum
    $maxY = ($solid | Measure-Object -Maximum).Maximum
    return [pscustomobject]@{ X = $minX; Y = $minY; Width = $maxX - $minX + 1; Height = $maxY - $minY + 1 }
}

function Get-DockEmptyPoint($pill) {
    return [pscustomobject]@{
        X = $pill.X + [int]($pill.Width / 2)
        Y = $pill.Y + (Convert-ToScaled 3)
    }
}

function Get-FillSampleRegion($box, $offset) {
    $centerX = $box.X + [int]($box.Width / 2)
    return [pscustomobject]@{
        X = $centerX - (Convert-ToScaled 2)
        Y = $box.Y + (Convert-ToScaled $offset[1])
        Width = Convert-ToScaled 4
        Height = Convert-ToScaled 2
    }
}

function Read-DockPreference([string]$path, [string]$key) {
    if (-not (Test-Path -LiteralPath $path)) { return $null }
    $line = Get-Content -LiteralPath $path | Where-Object { $_ -like "$key=*" } | Select-Object -First 1
    if (-not $line) { return $null }
    return $line.Substring($key.Length + 1)
}

function Run-VisualReferenceFlow {
    $manifestPath = Join-Path $repoRoot 'tests\visual\reference\manifest.json'
    $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
    $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize'
    $original = Get-ItemProperty -Path $key
    $process = $null
    $preferencePath = Join-Path $env:LOCALAPPDATA 'otto-os\plugins.txt'
    $preferenceBackup = if (Test-Path -LiteralPath $preferencePath) { [IO.File]::ReadAllBytes($preferencePath) } else { $null }
    try {
        Set-DockMagnification $preferencePath $false
        $process = Start-Otto 'visual-reference'
        $bar = Get-BarMetrics $process
        $dock = Get-OttoWindow $process.Id 'OttoDockBar' $true
        if (-not $dock) {
            throw 'OttoDockBar is not visible.'
        }
        $currentDpi = [OttoE2ENative]::DpiForWindow($bar.Handle)
        $currentWallpaper = (Get-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name Wallpaper -ErrorAction SilentlyContinue).Wallpaper
        if ($RecordReference) {
            $manifest.recordedWallpaper = $currentWallpaper
        }
        elseif ($manifest.recordedDpi -ne $currentDpi) {
            throw "Display DPI is $currentDpi but the reference was recorded at $($manifest.recordedDpi); record it again on this display."
        }
        elseif ($manifest.recordedWallpaper -ne $currentWallpaper) {
            throw 'The wallpaper differs from the one the reference was recorded with; record it again or restore the wallpaper.'
        }
        $modes = @(
            [pscustomobject]@{ Name = 'dark-opaque'; Light = 0 },
            [pscustomobject]@{ Name = 'light-opaque'; Light = 1 }
        )
        foreach ($mode in $modes) {
            Set-PersonalizeValue 'AppsUseLightTheme' $mode.Light
            Set-PersonalizeValue 'EnableTransparency' 0
            [OttoE2ESystem]::BroadcastSettingChange('ImmersiveColorSet')
            Start-Sleep -Milliseconds 2500
            $shot = Save-Screenshot 'visual-reference' $mode.Name
            $modeEntry = $manifest.modes.($mode.Name)

            $barColor = Get-RegionColor $shot (Get-EmptyBarRegion $bar)
            $notchRegion = [pscustomobject]@{ X = $bar.Left + [int]($bar.Width / 2) - (Convert-ToScaled 2); Y = $bar.Top + (Convert-ToScaled 4); Width = Convert-ToScaled 4; Height = Convert-ToScaled 2 }
            $notchColor = Get-RegionColor $shot $notchRegion

            $pill = Find-DockPill $shot $dock $modeEntry.'dock-fill-hint' $manifest.tolerance
            $dockRegion = Get-FillSampleRegion $pill $modeEntry.'dock-pill-offset'
            $dockColor = Get-RegionColor $shot $dockRegion
            $wallpaperRegion = [pscustomobject]@{ X = $dock.Left + (Convert-ToScaled 4); Y = $dock.Top + (Convert-ToScaled 4); Width = Convert-ToScaled 4; Height = Convert-ToScaled 2 }
            $wallpaperColor = Get-RegionColor $shot $wallpaperRegion

            $menuPoint = Get-DockEmptyPoint $pill
            Show-DockAt ([pscustomobject]@{ X = $menuPoint.X; Y = $menuPoint.Y; Edge = $dock.Bottom - 1 })
            Start-Sleep -Milliseconds 300
            [OttoE2ESystem]::RightClick($menuPoint.X, $menuPoint.Y)
            Wait-VisibleWindow $process 'OttoPopupMenu' "Dock settings menu for $($mode.Name)" 3000
            Start-Sleep -Milliseconds 500
            $menuShot = Save-Screenshot 'visual-reference' "$($mode.Name)-menu"
            $menuBox = Find-FillBox $menuShot $dock.Left ([Math]::Max(0, $dock.Top - (Convert-ToScaled 260))) $dock.Width (Convert-ToScaled 260) $modeEntry.'menu-fill-hint' $manifest.tolerance 'Dock settings menu'
            $menuRegion = Get-FillSampleRegion $menuBox $modeEntry.'menu-fill-offset'
            $menuColor = Get-RegionColor $menuShot $menuRegion
            [OttoE2ENative]::SendKey(0x1B)
            Wait-HiddenWindow $process 'OttoPopupMenu' "Dock settings menu to close for $($mode.Name)" 3000

            $barObserved = "rgb=$($barColor.R),$($barColor.G),$($barColor.B) spread=$($barColor.Spread)"
            $notchObserved = "rgb=$($notchColor.R),$($notchColor.G),$($notchColor.B) spread=$($notchColor.Spread)"
            $dockObserved = "rgb=$($dockColor.R),$($dockColor.G),$($dockColor.B) spread=$($dockColor.Spread) pill=$($pill.X),$($pill.Y),$($pill.Width)x$($pill.Height)"
            $menuObserved = "rgb=$($menuColor.R),$($menuColor.G),$($menuColor.B) spread=$($menuColor.Spread) box=$($menuBox.X),$($menuBox.Y),$($menuBox.Width)x$($menuBox.Height)"

            $surfaces = [ordered]@{
                'bar-empty' = @($barColor, $barObserved)
                'notch-core' = @($notchColor, $notchObserved)
                'dock-pill' = @($dockColor, $dockObserved)
                'menu-fill' = @($menuColor, $menuObserved)
            }

            if ($RecordReference) {
                foreach ($surface in $surfaces.Keys) {
                    $color = $surfaces[$surface][0]
                    $modeEntry.$surface = @($color.R, $color.G, $color.B)
                    Write-Output "REFERENCE_RECORDED mode=$($mode.Name) surface=$surface $($surfaces[$surface][1])"
                }
                continue
            }

            foreach ($surface in $surfaces.Keys) {
                $color = $surfaces[$surface][0]
                $observed = $surfaces[$surface][1]
                $expected = $modeEntry.$surface
                if ($null -eq $expected) {
                    throw "$($mode.Name)/${surface}: no reference recorded; run with -RecordReference first."
                }
                if (-not (Test-ColorNear $color @($expected) $manifest.tolerance) -or $color.Spread -gt $manifest.spreadMax) {
                    throw "$($mode.Name)/${surface}: colour drifted from the reference ($observed, expected $(@($expected) -join ','))."
                }
                Write-Output "CHECKPOINT_PASS scenario=visual-reference name=$($mode.Name)/$surface $observed"
            }
            if (Test-ColorNear $dockColor @($wallpaperColor.R, $wallpaperColor.G, $wallpaperColor.B) 24) {
                throw "$($mode.Name)/dock-pill: the sample matches the wallpaper outside the pill ($dockObserved), so the Dock fill was not measured."
            }
        }
        if ($RecordReference) {
            $manifest.recordedAt = (Get-Date).ToUniversalTime().ToString('o')
            $manifest.recordedDpi = $currentDpi
            $manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $manifestPath -Encoding UTF8
            Write-Output "REFERENCE_WRITTEN path=$manifestPath"
        }
    }
    finally {
        Set-PersonalizeValue 'AppsUseLightTheme' $original.AppsUseLightTheme
        Set-PersonalizeValue 'EnableTransparency' $original.EnableTransparency
        [OttoE2ESystem]::BroadcastSettingChange('ImmersiveColorSet')
        if ($script:ownedProcess) { Stop-Otto $script:ownedProcess }
        if ($preferenceBackup) {
            [IO.File]::WriteAllBytes($preferencePath, $preferenceBackup)
        }
        elseif (Test-Path -LiteralPath $preferencePath) {
            Remove-Item -LiteralPath $preferencePath -Force
        }
    }
}

function Set-DockMagnification([string]$path, [bool]$enabled) {
    $value = if ($enabled) { 'true' } else { 'false' }
    $lines = if (Test-Path -LiteralPath $path) { @(Get-Content -LiteralPath $path) } else { @() }
    $found = $false
    $updated = foreach ($line in $lines) {
        if ($line -like 'dock_magnification=*') { $found = $true; "dock_magnification=$value" } else { $line }
    }
    if (-not $found) { $updated = @($updated) + "dock_magnification=$value" }
    $directory = Split-Path -Parent $path
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
    Set-Content -LiteralPath $path -Value $updated -Encoding UTF8
}

function Run-DockSettingsFlow {
    $preferencePath = Join-Path $env:LOCALAPPDATA 'otto-os\plugins.txt'
    $backup = if (Test-Path -LiteralPath $preferencePath) { [IO.File]::ReadAllBytes($preferencePath) } else { $null }
    $process = $null
    try {
        Set-DockMagnification $preferencePath $false
        $process = Start-Otto 'dock-settings'
        $bar = Get-BarMetrics $process
        $dock = Get-OttoWindow $process.Id 'OttoDockBar' $true
        if (-not $dock) { throw 'OttoDockBar is not visible.' }
        $shot = Save-Screenshot 'dock-settings' '00-before'
        $light = [int](Get-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize' -Name AppsUseLightTheme).AppsUseLightTheme
        $hint = if ($light -eq 1) { @(245, 245, 250) } else { @(33, 33, 41) }
        $manifest = Get-Content -Raw -LiteralPath (Join-Path $repoRoot 'tests\visual\reference\manifest.json') | ConvertFrom-Json
        $pill = Find-DockPill $shot $dock $hint $manifest.tolerance
        $point = Get-DockEmptyPoint $pill

        $before = Read-DockPreference $preferencePath 'dock_magnification'
        Show-DockAt ([pscustomobject]@{ X = $point.X; Y = $point.Y; Edge = $dock.Bottom - 1 })
        Start-Sleep -Milliseconds 300
        [OttoE2ESystem]::RightClick($point.X, $point.Y)
        Wait-VisibleWindow $process 'OttoPopupMenu' 'Dock settings menu' 3000
        Start-Sleep -Milliseconds 400
        Save-Screenshot 'dock-settings' '01-menu-open' | Out-Null
        [OttoE2ENative]::SendKey(0x28)
        Start-Sleep -Milliseconds 150
        [OttoE2ENative]::SendKey(0x28)
        Start-Sleep -Milliseconds 150
        [OttoE2ENative]::SendKey(0x0D)
        Wait-HiddenWindow $process 'OttoPopupMenu' 'Dock settings menu to close after the choice' 3000
        Start-Sleep -Milliseconds 800
        $after = Read-DockPreference $preferencePath 'dock_magnification'
        if ($null -eq $after -or $after -eq $before) {
            throw "Choosing Magnification did not change plugins.txt (before=$before after=$after)."
        }
        Write-Output "CHECKPOINT_PASS scenario=dock-settings name=01-saved magnification $before -> $after"

        Stop-Otto $process
        $process = Start-Otto 'dock-settings'
        $persisted = Read-DockPreference $preferencePath 'dock_magnification'
        if ($persisted -ne $after) {
            throw "Dock magnification did not survive a restart (saved=$after reloaded=$persisted)."
        }
        Write-Output "CHECKPOINT_PASS scenario=dock-settings name=02-persisted-after-restart magnification=$persisted"
    }
    finally {
        if ($backup) {
            [IO.File]::WriteAllBytes($preferencePath, $backup)
        }
        elseif (Test-Path -LiteralPath $preferencePath) {
            Remove-Item -LiteralPath $preferencePath -Force
        }
        if ($script:ownedProcess) { Stop-Otto $script:ownedProcess }
    }
}

function Run-AppearanceFlow {
    $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize'
    $original = Get-ItemProperty -Path $key
    $originalLight = $original.AppsUseLightTheme
    $originalTransparency = $original.EnableTransparency
    Write-Output "APPEARANCE_ORIGINAL AppsUseLightTheme=$originalLight EnableTransparency=$originalTransparency"
    $process = $null
    try {
        $process = Start-Otto 'appearance'
        $bar = Get-BarMetrics $process
        $region = Get-EmptyBarRegion $bar
        $states = @(
            [pscustomobject]@{ Name = '01-dark-opaque'; Light = 0; Transparency = 0; Expected = @(28, 28, 33) },
            [pscustomobject]@{ Name = '02-light-opaque'; Light = 1; Transparency = 0; Expected = @(240, 240, 245) },
            [pscustomobject]@{ Name = '03-dark-glass'; Light = 0; Transparency = 1; Expected = $null }
        )
        foreach ($state in $states) {
            Set-PersonalizeValue 'AppsUseLightTheme' $state.Light
            Set-PersonalizeValue 'EnableTransparency' $state.Transparency
            [OttoE2ESystem]::BroadcastSettingChange('ImmersiveColorSet')
            Start-Sleep -Milliseconds 1500
            $shot = Save-Screenshot 'appearance' $state.Name
            $color = Get-RegionColor $shot $region
            $observed = "rgb=$($color.R),$($color.G),$($color.B) spread=$($color.Spread)"
            if ($state.Expected) {
                if (-not (Test-ColorNear $color $state.Expected 4) -or $color.Spread -gt 6) {
                    throw "$($state.Name): the bar did not switch to the opaque surface ($observed, expected $($state.Expected -join ','))."
                }
            }
            elseif ((Test-ColorNear $color @(28, 28, 33) 4) -or (Test-ColorNear $color @(240, 240, 245) 4)) {
                throw "$($state.Name): the bar stayed opaque after transparency was turned back on ($observed)."
            }
            Write-Output "CHECKPOINT_PASS scenario=appearance name=$($state.Name) $observed"
        }
    }
    finally {
        Set-PersonalizeValue 'AppsUseLightTheme' $originalLight
        Set-PersonalizeValue 'EnableTransparency' $originalTransparency
        [OttoE2ESystem]::BroadcastSettingChange('ImmersiveColorSet')
        $restored = Get-ItemProperty -Path $key
        Write-Output "APPEARANCE_RESTORED AppsUseLightTheme=$($restored.AppsUseLightTheme) EnableTransparency=$($restored.EnableTransparency)"
    }
    Stop-Otto $process
}

function Run-AudioFlow {
    $devices = [OttoE2EAudio]::ActiveRenderDevices()
    $original = [OttoE2EAudio]::Volume()
    Write-Output "AUDIO_ORIGINAL volume=$([Math]::Round($original, 3)) active_render_devices=$devices"
    $process = Start-Otto 'audio'
    try {
        $bar = Get-BarMetrics $process
        $region = [pscustomobject]@{
            X = $bar.Left + [int]($bar.Width / 2) - (Convert-ToScaled 300)
            Y = $bar.Top
            Width = Convert-ToScaled 600
            Height = $bar.Height
        }
        Start-Sleep -Milliseconds 2500
        $baseline = Save-Screenshot 'audio' '00-baseline'
        $target = if ($original -gt 0.5) { $original - 0.12 } else { $original + 0.12 }
        $timer = [Diagnostics.Stopwatch]::StartNew()
        [OttoE2EAudio]::SetVolume([float]$target)
        $shown = $false
        $changed = 0
        while ($timer.ElapsedMilliseconds -lt 2000 -and -not $shown) {
            $shot = Save-Screenshot 'audio' '01-hud'
            $changed = Get-RegionDiff $baseline $shot $region
            $shown = $changed -ge 150
        }
        $latency = $timer.ElapsedMilliseconds
        if (-not $shown) {
            throw "An external volume change did not raise the Otto HUD within 2 s (changed pixels: $changed)."
        }
        Write-Output "CHECKPOINT_PASS scenario=audio name=01-external-volume-hud latency_ms=$latency changed=$changed"
    }
    finally {
        [OttoE2EAudio]::SetVolume([float]$original)
        Write-Output "AUDIO_RESTORED volume=$([Math]::Round([OttoE2EAudio]::Volume(), 3))"
    }
    Stop-Otto $process
}

$scenarios = if ($Scenario -eq 'all') { @('success', 'false', 'edge', 'worst', 'fullscreen', 'genie', 'native-minimize', 'window-cover', 'dock', 'keyboard', 'keyboard-dock', 'stall') } else { @($Scenario) }
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
                'recovery' { Run-RecoveryFlow }
                'fullscreen' { Run-FullScreenFlow }
                'genie' { Run-GenieFlow }
                'dock' { Run-DockFlow }
                'keyboard' { Run-KeyboardFlow }
                'keyboard-dock' { Run-KeyboardDockFlow }
                'stall' { Run-StallFlow }
                'appearance' { Run-AppearanceFlow }
                'visual-reference' { Run-VisualReferenceFlow }
                'dock-settings' { Run-DockSettingsFlow }
                'native-minimize' { Run-NativeMinimizeFlow }
                'window-cover' { Run-WindowCoverFlow }
                'audio' { Run-AudioFlow }
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
