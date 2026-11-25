Add-Type -AssemblyName System.Drawing

$srcPath = "C:\GitHub\s2x3\.design\s2x3_AppIcon.png"
$src = [System.Drawing.Image]::FromFile($srcPath)

function Resize-Save([System.Drawing.Image]$image, [int]$size, [string]$dest) {
  $bmp = New-Object System.Drawing.Bitmap($size, $size)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
  $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $g.DrawImage($image, 0, 0, $size, $size)
  $g.Dispose()
  $bmp.Save($dest, [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
}

# 1) site, desktop UI, docs: full-resolution original
Copy-Item $srcPath "C:\GitHub\s2x3\site\assets\icon.png" -Force
Copy-Item $srcPath "C:\GitHub\s2x3\apps\desktop\src\assets\icon.png" -Force
Copy-Item $srcPath "C:\GitHub\s2x3\docs\assets\icon.png" -Force

# 2) tauri icon set
$icons = "C:\GitHub\s2x3\apps\desktop\src-tauri\icons"
Resize-Save $src 32  "$icons\32x32.png"
Resize-Save $src 128 "$icons\128x128.png"
Resize-Save $src 256 "$icons\128x128@2x.png"
Resize-Save $src 256 "$icons\256x256.png"
Resize-Save $src 512 "$icons\512x512.png"
Resize-Save $src 512 "$icons\icon-512.png"
Resize-Save $src 512 "$icons\icon.png"

# 3) multi-size .ico with embedded PNGs (16..256)
$sizes = @(16, 24, 32, 48, 64, 128, 256)
$pngs = @()
foreach ($s in $sizes) {
  $tmp = "$env:TEMP\ico_$s.png"
  Resize-Save $src $s $tmp
  $pngs += ,@($s, [System.IO.File]::ReadAllBytes($tmp))
  Remove-Item $tmp
}

$ms = New-Object System.IO.MemoryStream
$bw = New-Object System.IO.BinaryWriter($ms)
$bw.Write([uint16]0)            # reserved
$bw.Write([uint16]1)            # type: icon
$bw.Write([uint16]$pngs.Count)  # count
$offset = 6 + 16 * $pngs.Count
foreach ($p in $pngs) {
  $s = $p[0]; $bytes = $p[1]
  $dim = if ($s -ge 256) { 0 } else { $s }
  $bw.Write([byte]$dim)   # width
  $bw.Write([byte]$dim)   # height
  $bw.Write([byte]0)      # palette
  $bw.Write([byte]0)      # reserved
  $bw.Write([uint16]1)    # planes
  $bw.Write([uint16]32)   # bit depth
  $bw.Write([uint32]$bytes.Length)
  $bw.Write([uint32]$offset)
  $offset += $bytes.Length
}
foreach ($p in $pngs) { $bw.Write($p[1]) }
$bw.Flush()
[System.IO.File]::WriteAllBytes("$icons\icon.ico", $ms.ToArray())
$bw.Dispose(); $ms.Dispose()

$src.Dispose()
Get-ChildItem $icons | Select-Object Name, Length
Get-Item "C:\GitHub\s2x3\site\assets\icon.png","C:\GitHub\s2x3\apps\desktop\src\assets\icon.png","C:\GitHub\s2x3\docs\assets\icon.png" | Select-Object FullName, Length
