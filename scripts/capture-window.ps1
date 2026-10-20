param(
    [string]$ProcessName = "mpcraft",
    [string]$Output = "mpcraft-window.png"
)

Add-Type -AssemblyName System.Drawing

$signature = @"
using System;
using System.Runtime.InteropServices;
using System.Drawing;

public class Capture {
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdcBlt, uint nFlags);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

if (-not ("Capture" -as [type])) {
    Add-Type -TypeDefinition $signature -ReferencedAssemblies System.Drawing
}

$process = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 } |
    Select-Object -First 1

if (-not $process) {
    Write-Error "no window found for $ProcessName"
    exit 1
}

$handle = $process.MainWindowHandle
[Capture]::ShowWindow($handle, 9) | Out-Null
[Capture]::SetForegroundWindow($handle) | Out-Null
Start-Sleep -Milliseconds 700

$rect = New-Object Capture+RECT
[Capture]::GetWindowRect($handle, [ref]$rect) | Out-Null

$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top

if ($width -le 0 -or $height -le 0) {
    Write-Error "the window has no size"
    exit 1
}

$bitmap = New-Object System.Drawing.Bitmap($width, $height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$hdc = $graphics.GetHdc()

# 2 = PW_RENDERFULLCONTENT, which renders the window even when it is covered.
$ok = [Capture]::PrintWindow($handle, $hdc, 2)

$graphics.ReleaseHdc($hdc)
$graphics.Dispose()

if (-not $ok) {
    Write-Error "PrintWindow failed"
    exit 1
}

$path = Join-Path (Get-Location).Path $Output
$bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
$bitmap.Dispose()

Write-Output "captured $width x $height to $path"
