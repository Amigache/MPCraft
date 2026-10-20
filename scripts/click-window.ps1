param(
    [string]$ProcessName = "mpcraft",
    [int]$X = 1096,
    [int]$Y = 24
)

$signature = @"
using System;
using System.Runtime.InteropServices;
public class Click {
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, IntPtr extra);
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

if (-not ("Click" -as [type])) {
    Add-Type -TypeDefinition $signature
}

$process = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 } |
    Select-Object -First 1

if (-not $process) {
    Write-Error "no window found for $ProcessName"
    exit 1
}

$handle = $process.MainWindowHandle
[Click]::ShowWindow($handle, 9) | Out-Null
[Click]::SetForegroundWindow($handle) | Out-Null
Start-Sleep -Milliseconds 500

$rect = New-Object Click+RECT
[Click]::GetWindowRect($handle, [ref]$rect) | Out-Null

$screenX = $rect.Left + $X
$screenY = $rect.Top + $Y

[Click]::SetCursorPos($screenX, $screenY) | Out-Null
Start-Sleep -Milliseconds 250

# The left button goes down and up.
[Click]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 60
[Click]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)

Write-Output "clicked at $screenX,$screenY (window $($rect.Left),$($rect.Top))"
