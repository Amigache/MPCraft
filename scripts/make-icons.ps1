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

function Write-BigEndian([System.IO.BinaryWriter]$writer, [UInt32]$value) {
    # ICNS keeps its lengths big-endian, while BinaryWriter writes little-endian.
    foreach ($shift in 24, 16, 8, 0) {
        $writer.Write([Byte](($value -shr $shift) -band 0xFF))
    }
}

function New-IconIcns([string]$icnsPath, [object[]]$chunks) {
    $body = New-Object System.IO.MemoryStream

    foreach ($chunk in $chunks) {
        $png = [System.IO.File]::ReadAllBytes($chunk.Path)
        $bw = New-Object System.IO.BinaryWriter($body, [System.Text.Encoding]::ASCII, $true)
        $bw.Write([System.Text.Encoding]::ASCII.GetBytes($chunk.Type))
        Write-BigEndian $bw ([UInt32](8 + $png.Length))
        $bw.Write($png)
        $bw.Flush()
        $bw.Dispose()
    }

    $bytes = $body.ToArray()
    $body.Dispose()

    $fs = [System.IO.File]::Create($icnsPath)
    $header = New-Object System.IO.BinaryWriter($fs)

    # The magic number and the length of the whole container, header included.
    $header.Write([System.Text.Encoding]::ASCII.GetBytes("icns"))
    Write-BigEndian $header ([UInt32](8 + $bytes.Length))
    $header.Write($bytes)
    $header.Flush()
    $header.Dispose()
    $fs.Dispose()
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
New-IconPng 256 (Join-Path $iconDir "128x128@2x.png")
New-IconPng 32 (Join-Path $iconDir "32x32.png")

# macOS reads an .icns container rather than a PNG, so every size the platform asks for is
# written into one: the points of the display and the twice-and-thrice forms Retina reads.
$sizes = @{
    16   = Join-Path $iconDir "16x16.png"
    32   = Join-Path $iconDir "32x32.png"
    64   = Join-Path $iconDir "64x64.png"
    128  = Join-Path $iconDir "128x128.png"
    256  = Join-Path $iconDir "128x128@2x.png"
    512  = $master
    1024 = Join-Path $iconDir "1024x1024.png"
}

New-IconPng 16 $sizes[16]
New-IconPng 64 $sizes[64]
New-IconPng 1024 $sizes[1024]

New-IconIcns (Join-Path $iconDir "icon.icns") @(
    @{ Type = "icp4"; Path = $sizes[16] },
    @{ Type = "icp5"; Path = $sizes[32] },
    @{ Type = "icp6"; Path = $sizes[64] },
    @{ Type = "ic07"; Path = $sizes[128] },
    @{ Type = "ic08"; Path = $sizes[256] },
    @{ Type = "ic09"; Path = $sizes[512] },
    @{ Type = "ic10"; Path = $sizes[1024] },
    @{ Type = "ic11"; Path = $sizes[32] },
    @{ Type = "ic12"; Path = $sizes[64] },
    @{ Type = "ic13"; Path = $sizes[256] },
    @{ Type = "ic14"; Path = $sizes[512] }
)

Write-Output "icons written to $iconDir"
Get-ChildItem $iconDir | Select-Object Name, Length
