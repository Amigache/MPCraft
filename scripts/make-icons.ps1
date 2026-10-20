Add-Type -AssemblyName System.Drawing

$iconDir = Join-Path $PSScriptRoot "..\apps\desktop\icons"
New-Item -ItemType Directory -Force -Path $iconDir | Out-Null

function New-IconPng([int]$size, [string]$path) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias

    $g.Clear([System.Drawing.Color]::FromArgb(255, 12, 16, 22))

    # A simple stylised rotor/propeller mark.
    $centre = New-Object System.Drawing.PointF(($size / 2), ($size / 2))
    $arm = $size * 0.34
    $thick = $size * 0.10

    $pen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(255, 0, 200, 150), $thick)
    $pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round

    foreach ($angle in 0, 90, 180, 270) {
        $rad = $angle * [Math]::PI / 180
        $end = New-Object System.Drawing.PointF(
            ($centre.X + $arm * [Math]::Cos($rad)),
            ($centre.Y + $arm * [Math]::Sin($rad)))
        $g.DrawLine($pen, $centre, $end)
    }

    $brush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 255, 190, 60))
    $r = $size * 0.09
    $g.FillEllipse($brush, ($centre.X - $r), ($centre.Y - $r), ($r * 2), ($r * 2))

    $g.Dispose()
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}

function New-IconFile([string]$pngPath, [string]$icoPath, [int]$size) {
    $png = [System.IO.File]::ReadAllBytes($pngPath)

    $fs = [System.IO.File]::Create($icoPath)
    $bw = New-Object System.IO.BinaryWriter($fs)

    # ICONDIR
    $bw.Write([UInt16]0)      # reserved
    $bw.Write([UInt16]1)      # type: icon
    $bw.Write([UInt16]1)      # image count

    # ICONDIRENTRY
    $bw.Write([Byte]($(if ($size -ge 256) { 0 } else { $size })))
    $bw.Write([Byte]($(if ($size -ge 256) { 0 } else { $size })))
    $bw.Write([Byte]0)        # palette size
    $bw.Write([Byte]0)        # reserved
    $bw.Write([UInt16]1)      # colour planes
    $bw.Write([UInt16]32)     # bits per pixel
    $bw.Write([UInt32]$png.Length)
    $bw.Write([UInt32]22)     # offset of the image data

    $bw.Write($png)
    $bw.Flush()
    $bw.Dispose()
    $fs.Dispose()
}

$master = Join-Path $iconDir "icon.png"
New-IconPng 512 $master
New-IconFile $master (Join-Path $iconDir "icon.ico") 256

New-IconPng 128 (Join-Path $iconDir "128x128.png")
New-IconPng 64 (Join-Path $iconDir "128x128@2x.png")

Write-Output "icons written to $iconDir"
Get-ChildItem $iconDir | Select-Object Name, Length
