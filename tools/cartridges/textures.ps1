param([string]$M = "$PSScriptRoot\sources", [string]$O = "$PSScriptRoot\..\..\app\public\models")
Add-Type -AssemblyName System.Drawing
$jpeg = [System.Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() | Where-Object { $_.MimeType -eq "image/jpeg" }

function Convert-Tex($src, $dst, $size, $quality, $fills) {
  $img = [System.Drawing.Image]::FromFile($src)
  $bmp = New-Object System.Drawing.Bitmap $size, $size
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $g.DrawImage($img, 0, 0, $size, $size)
  foreach ($f in $fills) {
    if ($f.sample) {
      $c = $bmp.GetPixel([int]($f.sample[0] * $size), [int]($f.sample[1] * $size))
    } else { $c = [System.Drawing.Color]::FromArgb($f.rgb[0], $f.rgb[1], $f.rgb[2]) }
    $brush = New-Object System.Drawing.SolidBrush $c
    $g.FillRectangle($brush, [single]($f.x0 * $size), [single]($f.y0 * $size), [single](($f.x1 - $f.x0) * $size), [single](($f.y1 - $f.y0) * $size))
    $brush.Dispose()
  }
  $g.Dispose()
  $params = New-Object System.Drawing.Imaging.EncoderParameters 1
  $params.Param[0] = [System.Drawing.Imaging.EncoderParameter]::new([System.Drawing.Imaging.Encoder]::Quality, [long]$quality)
  $bmp.Save($dst, $jpeg, $params)
  $bmp.Dispose(); $img.Dispose()
  "{0} {1} Ko" -f $dst, [int]((Get-Item $dst).Length / 1024)
}

# Texte « GAME BOY ADVANCE SP » moulé sous l'étiquette : faux (« SP ») sur une cartouche, effacé.
$sp = @{ x0 = 0.17; y0 = 0.6; x1 = 0.45; y1 = 0.665 }
# Étiquettes d'origine (Club Penguin, Tomodachi Life, Rouge Feu, Octopath) remplacées par du papier vierge.
$paper = @(232, 231, 226)
Convert-Tex "$M\ds_game_card\textures\lambert1_baseColor.png" "$O\ds\base.jpg" 1024 88 @(@{ x0 = 0.2975; y0 = 0.405; x1 = 0.6125; y1 = 0.747; rgb = $paper })
Convert-Tex "$M\ds_game_card\textures\lambert1_metallicRoughness.png" "$O\ds\mr.jpg" 512 88 @()
Convert-Tex "$M\3ds_game_cartridge\textures\lambert2_baseColor.png" "$O\3ds\base.jpg" 1024 88 @(@{ x0 = 0.435; y0 = 0.509; x1 = 0.833; y1 = 0.966; rgb = $paper })
Convert-Tex "$M\3ds_game_cartridge\textures\lambert2_normal.png" "$O\3ds\normal.jpg" 1024 92 @()
Convert-Tex "$M\3ds_game_cartridge\textures\lambert2_metallicRoughness.png" "$O\3ds\mr.jpg" 64 90 @()
Convert-Tex "$M\3ds_game_cartridge\textures\lambert3_baseColor.png" "$O\3ds\chip.jpg" 512 85 @()
Convert-Tex "$M\3ds_game_cartridge\textures\lambert3_normal.png" "$O\3ds\chip-normal.jpg" 256 90 @()
Convert-Tex "$M\3ds_game_cartridge\textures\lambert3_metallicRoughness.png" "$O\3ds\chip-mr.jpg" 64 90 @()
Convert-Tex "$M\pokemon_cartridge_gameboy\textures\Cartridge_MAT_baseColor.png" "$O\gba\base.jpg" 2048 88 @(@{ x0 = 0.1; y0 = 0.39; x1 = 0.508; y1 = 0.607; rgb = $paper }, @{ x0 = $sp.x0; y0 = $sp.y0; x1 = $sp.x1; y1 = $sp.y1; sample = @(0.08, 0.64) })
Convert-Tex "$M\pokemon_cartridge_gameboy\textures\Cartridge_MAT_normal.png" "$O\gba\normal.jpg" 2048 90 @(@{ x0 = $sp.x0; y0 = $sp.y0; x1 = $sp.x1; y1 = $sp.y1; rgb = @(128, 128, 255) })
Convert-Tex "$M\pokemon_cartridge_gameboy\textures\Cartridge_MAT_metallicRoughness.png" "$O\gba\mr.jpg" 1024 88 @(@{ x0 = $sp.x0; y0 = $sp.y0; x1 = $sp.x1; y1 = $sp.y1; sample = @(0.08, 0.64) })
Convert-Tex "$M\nintendo_switch_game_cartridge_v2\textures\Material.001_baseColor.png" "$O\switch\label.jpg" 256 90 @(@{ x0 = 0.07; y0 = 0.05; x1 = 0.93; y1 = 0.85; rgb = $paper })
