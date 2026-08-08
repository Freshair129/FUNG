Add-Type -AssemblyName System.Drawing

$srcPath = "D:\FUNG\docs\FUNG LOGO - Quiet Archive.png"
$destPath = "D:\FUNG\src-tauri\icons\icon.png"

$src = [System.Drawing.Bitmap]::FromFile($srcPath)

# In "FUNG LOGO - Quiet Archive.png", the large centered logo is roughly between Y=5% to 45% and X=38% to 95%
$cropWidth = [int]($src.Width * 0.52)
$cropHeight = [int]($src.Height * 0.40)
$cropX = [int]($src.Width * 0.42)
$cropY = [int]($src.Height * 0.08)

$cropRect = New-Object System.Drawing.Rectangle($cropX, $cropY, $cropWidth, $cropHeight)
$cropped = $src.Clone($cropRect, $src.PixelFormat)

$size = 1024
$finalBmp = New-Object System.Drawing.Bitmap($size, $size)
$g = [System.Drawing.Graphics]::FromImage($finalBmp)
$g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
$g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality

$bgColor = [System.Drawing.ColorTranslator]::FromHtml("#171918")
$g.Clear($bgColor)

# Draw cropped logo centered
$destX = [int]($size * 0.1)
$destY = [int]($size * 0.1)
$destW = [int]($size * 0.8)
$destH = [int]($size * 0.8)

$g.DrawImage($cropped, $destX, $destY, $destW, $destH)

$finalBmp.Save($destPath, [System.Drawing.Imaging.ImageFormat]::Png)

$cropped.Dispose()
$src.Dispose()
$finalBmp.Dispose()
$g.Dispose()

Write-Host "App icon generated successfully at $destPath"
