# Generate the four Xbox glyph tiles (dark circle, colored letter) used by
# the launcher's automatic texture-patch generator. 80x80 PNGs.
Add-Type -AssemblyName System.Drawing

$out = "C:\GitHub\s2x3\apps\desktop\src-tauri\assets\glyphs"
New-Item -ItemType Directory -Force -Path $out | Out-Null

$glyphs = @(
  @{ L = "a"; C = [System.Drawing.Color]::FromArgb(255, 63, 191, 95) },
  @{ L = "b"; C = [System.Drawing.Color]::FromArgb(255, 229, 72, 77) },
  @{ L = "x"; C = [System.Drawing.Color]::FromArgb(255, 59, 130, 246) },
  @{ L = "y"; C = [System.Drawing.Color]::FromArgb(255, 242, 199, 68) }
)

foreach ($g in $glyphs) {
  $size = 80
  $bmp = New-Object System.Drawing.Bitmap($size, $size)
  $gfx = [System.Drawing.Graphics]::FromImage($bmp)
  $gfx.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $gfx.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
  $gfx.Clear([System.Drawing.Color]::Transparent)
  $brush = New-Object System.Drawing.SolidBrush($g.C)
  $gfx.FillEllipse($brush, 0, 0, $size - 1, $size - 1)
  $font = New-Object System.Drawing.Font("Arial", 52, [System.Drawing.FontStyle]::Bold)
  $fmt = New-Object System.Drawing.StringFormat
  $fmt.Alignment = [System.Drawing.StringAlignment]::Center
  $fmt.LineAlignment = [System.Drawing.StringAlignment]::Center
  $rect = New-Object System.Drawing.RectangleF(0, -2, $size, $size)
  $gfx.DrawString($g.L.ToUpper(), $font, [System.Drawing.Brushes]::White, $rect, $fmt)
  $gfx.Dispose()
  $p = Join-Path $out "$($g.L).png"
  $bmp.Save($p, [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
  "wrote $p"
}
