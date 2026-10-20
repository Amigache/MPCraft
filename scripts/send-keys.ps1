param(
    [string]$ProcessName = "mpcraft",
    [int]$Tabs = 1,
    [string]$Keys = "{ENTER}"
)

Add-Type -AssemblyName System.Windows.Forms

$signature = @"
using System;
using System.Runtime.InteropServices;
public class Focus {
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
}
"@

if (-not ("Focus" -as [type])) {
    Add-Type -TypeDefinition $signature
}

$process = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 } |
    Select-Object -First 1

if (-not $process) {
    Write-Error "no window found for $ProcessName"
    exit 1
}

[Focus]::ShowWindow($process.MainWindowHandle, 9) | Out-Null
[Focus]::SetForegroundWindow($process.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 700

for ($i = 0; $i -lt $Tabs; $i++) {
    [System.Windows.Forms.SendKeys]::SendWait("{TAB}")
    Start-Sleep -Milliseconds 150
}

[System.Windows.Forms.SendKeys]::SendWait($Keys)
Start-Sleep -Milliseconds 700

Write-Output "sent $Tabs tabs and $Keys to $ProcessName"
