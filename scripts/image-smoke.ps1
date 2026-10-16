param([ValidateSet('light','dark')][string]$Theme = 'light', [string]$Exe = "$PSScriptRoot\..\target\release\feathermd.exe", [string]$Output = "$PSScriptRoot\..\test-artifacts\image", [string]$Server = "$PSScriptRoot\..\target\release\examples\update_server.exe")
# Local images: off by default; when on, only local raster files are drawn (fitted,
# EXIF-oriented, transparent where they are), everything else keeps a placeholder
# that says why. Manual HTTP loads use generated fixtures on loopback only.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class ImageSmoke {
 public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr data);
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr hwnd, EnumProc proc, IntPtr data);
 [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")] static extern IntPtr ReadText(IntPtr hwnd, uint msg, IntPtr wp, StringBuilder text);
 public static string Content(IntPtr hwnd) {
  var text = new StringBuilder();
  EnumChildWindows(hwnd, (h,d) => { var s = new StringBuilder(4096); ReadText(h, 13, (IntPtr)s.Capacity, s); text.AppendLine(s.ToString()); return true; }, IntPtr.Zero);
  return text.ToString();
 }
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr hwnd, StringBuilder text, int size);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int width, int height, uint flags);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process, uint flag);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr hwnd, int id);
 [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd, int command);
 public static IntPtr Find(uint pid) { return FindClass(pid, "FeatherMD.Native.Reader"); }
 public static IntPtr FindClass(uint pid, string name) {
  IntPtr found = IntPtr.Zero;
  EnumWindows((h,d) => { uint p; GetWindowThreadProcessId(h, out p); var c = new StringBuilder(128); GetClassNameW(h, c, c.Capacity); if (p == pid && c.ToString() == name) { found = h; return false; } return true; }, IntPtr.Zero);
  return found;
 }
}
"@
New-Item -ItemType Directory -Path $Output -Force | Out-Null
$Output = (Resolve-Path -LiteralPath $Output).Path
$Exe = (Resolve-Path -LiteralPath $Exe).Path
$Server = (Resolve-Path -LiteralPath $Server).Path
$ini = Join-Path (Split-Path $Exe) 'FeatherMD.ini'
if (Test-Path -LiteralPath $ini) { throw 'Refusing to overwrite existing portable settings' }
$utf8 = [Text.UTF8Encoding]::new($false)
$oldSnapshot = $env:FEATHERMD_TEST_SNAPSHOT
$snapshot = Join-Path $Output 'image.bmp'
$copyFile = Join-Path $Output 'copy.txt'
$process = $null; $window = [IntPtr]::Zero; $createdIni = $false; $dpi = 96
$serverProcess = $null
$feed = Join-Path $Output ('web-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $feed | Out-Null
$oldDpi = [ImageSmoke]::SetThreadDpiAwarenessContext([IntPtr](-4))

# Fixtures.
$assets = Join-Path $Output 'assets'
New-Item -ItemType Directory -Path $assets -Force | Out-Null
function SaveBitmap([int]$width, [int]$height, [scriptblock]$paint, [string]$path, [Drawing.Imaging.ImageFormat]$format) {
 $bitmap = [Drawing.Bitmap]::new($width, $height, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
 $g = [Drawing.Graphics]::FromImage($bitmap)
 try { & $paint $g } finally { $g.Dispose() }
 try { $bitmap.Save($path, $format) } finally { $bitmap.Dispose() }
}
function Fill($g, [int[]]$rgba, [int]$x, [int]$y, [int]$w, [int]$h) {
 $brush = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb($rgba[3], $rgba[0], $rgba[1], $rgba[2]))
 try { $g.FillRectangle($brush, $x, $y, $w, $h) } finally { $brush.Dispose() }
}
# 240x120: red left, blue right, transparent bottom sixth.
SaveBitmap 240 120 { param($g) $g.Clear([Drawing.Color]::Transparent); Fill $g @(220, 40, 40, 255) 0 0 120 100; Fill $g @(40, 80, 220, 255) 120 0 120 100 } (Join-Path $assets 'halves.png') ([Drawing.Imaging.ImageFormat]::Png)
# Stored 40x20, green left and white right, then EXIF orientation 6 (view rotated
# 90 degrees clockwise): shown 20x40 with green on top.
$plain = Join-Path $Output 'rotated-plain.jpg'
SaveBitmap 40 20 { param($g) Fill $g @(255, 255, 255, 255) 0 0 40 20; Fill $g @(20, 160, 40, 255) 0 0 20 20 } $plain ([Drawing.Imaging.ImageFormat]::Jpeg)
$jpeg = [IO.File]::ReadAllBytes($plain)
$app1 = [byte[]](0xff,0xe1,0x00,0x22) + [Text.Encoding]::ASCII.GetBytes("Exif") + [byte[]](0,0) + [byte[]](0x49,0x49,0x2a,0x00,0x08,0,0,0) + [byte[]](1,0,0x12,0x01,3,0,1,0,0,0,6,0,0,0,0,0,0,0)
[IO.File]::WriteAllBytes((Join-Path $assets 'rotated.jpg'), [byte[]]($jpeg[0..1] + $app1 + $jpeg[2..($jpeg.Length - 1)]))
Remove-Item -LiteralPath $plain
# A wide photo that must shrink to the column.
SaveBitmap 2400 300 { param($g) Fill $g @(30, 140, 90, 255) 0 0 2400 300 } (Join-Path $assets 'wide.jpg') ([Drawing.Imaging.ImageFormat]::Jpeg)
foreach ($badge in @(@('build', @(40, 160, 70, 255)), @('docs', @(30, 110, 200, 255)))) {
 $color = $badge[1]
 SaveBitmap 90 20 { param($g) Fill $g $color 0 0 90 20 } (Join-Path $assets "badge-$($badge[0]).png") ([Drawing.Imaging.ImageFormat]::Png)
}
[IO.File]::WriteAllText((Join-Path $assets 'broken.png'), 'this is not a png', $utf8)
[IO.File]::WriteAllText((Join-Path $assets 'logo.svg'), '<svg xmlns="http://www.w3.org/2000/svg"/>', $utf8)
# A junction to the assets: refused, since a link could point at a share.
$junction = Join-Path $Output 'linked'
if (Test-Path -LiteralPath $junction) { [IO.Directory]::Delete($junction) }
New-Item -ItemType Junction -Path $junction -Target $assets | Out-Null
$haveJunction = Test-Path -LiteralPath (Join-Path $junction 'halves.png')
$document = Join-Path $Output 'images.md'
[IO.File]::WriteAllLines($document, [string[]]@(
 '# 图片'
 ''
 '<p align="center"><img src="assets/halves.png" alt="红蓝" width="160"></p>'
 ''
 '![旋转](assets/rotated.jpg)'
 ''
 '[![build](assets/badge-build.png)](https://example.com/build) ![docs](assets/badge-docs.png)'
 ''
 '![宽图](assets/wide.jpg)'
 ''
 '![网络](https://example.com/logo.png)'
 ''
 '![链接](linked/halves.png)'
 ''
 '![缺失](assets/missing.png)'
 ''
 '![坏文件](assets/broken.png)'
 ''
 '![矢量](assets/logo.svg)'
 ''
 '结尾。'
), $utf8)
function Query([int]$id, [int]$index = 0) { [ImageSmoke]::SendMessageW($window, 0x802a, [IntPtr]$id, [IntPtr]$index).ToInt64() }
function Point([int]$x, [int]$y) { return [int](($y -shl 16) -bor ($x -band 0xffff)) }
function Send([uint32]$msg, [int]$wp, [int]$lp) { [void][ImageSmoke]::SendMessageW($window, $msg, [IntPtr]$wp, [IntPtr]$lp) }
function Scale([int]$value) { [int][Math]::Floor($value * $dpi / 96) }
# Layout settled and every image of the page read.
function Idle {
 for ($i = 0; $i -lt 800; $i++) {
  if ($process.HasExited) { throw 'Reader exited unexpectedly' }
  if ((Query 11) -eq 0 -and (Query 2) -gt 0 -and (Query 82) -eq 0) {
   # The timer that takes finished images runs every 30 ms; let it lay out.
   Start-Sleep -Milliseconds 80
   if ((Query 82) -eq 0) { return }
  }
  Start-Sleep -Milliseconds 10
 }
 throw "Images did not finish loading: $(Query 82) pending"
}
function StartReader([string]$settings) {
 [IO.File]::WriteAllText($ini, "theme=$Theme`nlanguage=zh-CN`nfont_size=16`n" + $settings, $utf8)
 $script:createdIni = $true
 $env:FEATHERMD_TEST_SNAPSHOT = $snapshot
 $script:process = Start-Process -FilePath $Exe -ArgumentList ('"' + $document + '"') -WindowStyle Hidden -PassThru
 $script:window = [IntPtr]::Zero
 for ($i = 0; $i -lt 200 -and $window -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 25; $script:window = [ImageSmoke]::Find($process.Id) }
 if ($window -eq [IntPtr]::Zero) { throw 'Reader window not created' }
 $script:dpi = 96
 Idle
 $script:dpi = [int](Query 3)
}
function StopReader {
 if ($process -and -not $process.HasExited) {
  [void][ImageSmoke]::PostMessageW($window, 0x10, [IntPtr]::Zero, [IntPtr]::Zero)
  if (-not $process.WaitForExit(5000)) { $process.Kill(); throw 'Reader failed to exit cleanly' }
 }
}
# Paints, then keeps the frame for pixel checks (and as a PNG when named).
function Capture([string]$name = '') {
 if ([ImageSmoke]::IsWindowVisible($window)) { throw 'Reader became visible' }
 if ((Query 4) -ne 1) { throw 'Internal capture failed' }
 if ($script:frame) { $script:frame.Dispose() }
 # A copy in memory: a Bitmap opened from the file would keep it locked, and the
 # next snapshot could not replace it.
 $file = [Drawing.Bitmap]::new($snapshot)
 try { $script:frame = [Drawing.Bitmap]::new($file) } finally { $file.Dispose() }
 if ($name) { $frame.Save((Join-Path $Output ($name + '.png')), [Drawing.Imaging.ImageFormat]::Png) }
}
function Painted([int]$index) {
 $r = @{ Left = (Query 85 $index); Top = (Query 86 $index); Right = (Query 87 $index); Bottom = (Query 88 $index) }
 if ($r.Left -lt 0) { throw "Image $index was not drawn" }
 $r.Width = $r.Right - $r.Left; $r.Height = $r.Bottom - $r.Top
 return $r
}
function Near([int]$value, [int]$expected, [int]$slack, [string]$what) {
 if ([Math]::Abs($value - $expected) -gt $slack) { throw "$what is $value, expected $expected" }
}
function Pixel([int]$x, [int]$y) { $c = $frame.GetPixel($x, $y); return @($c.R, $c.G, $c.B) }
# Select all, copy through the test seam, then Esc clears the selection again.
function Copied {
 Send 0x111 108 0
 $bytes = Query 62
 Send 0x100 0x1b 0
 if ($bytes -lt 0) { throw 'Copy seam unavailable' }
 return [IO.File]::ReadAllText($copyFile, $utf8)
}
function StableGdi {
 $min = [int]::MaxValue
 for ($j = 0; $j -lt 12; $j++) { Start-Sleep -Milliseconds 5; $min = [Math]::Min($min, [ImageSmoke]::GetGuiResources($process.Handle, 0)) }
 return $min
}
$frame = $null
$suffix = if ($Theme -eq 'dark') { '-dark' } else { '' }
function Requests {
 $log = Join-Path $feed 'requests.log'
 if (Test-Path -LiteralPath $log) { return @([IO.File]::ReadAllLines($log) | Where-Object { $_.StartsWith('GET ') }).Count }
 return 0
}
function LoadWeb([bool]$accept) {
 Capture
 if ((Query 92) -lt 1) { throw 'No manual web-image target was painted' }
 $point = Point ([int](Query 93) + 4) ([int](Query 94) + 4)
 Send 0x201 1 $point
 [void][ImageSmoke]::PostMessageW($window, 0x202, [IntPtr]::Zero, [IntPtr]$point)
 $confirm = [IntPtr]::Zero
 for ($i = 0; $i -lt 200; $i++) {
  $confirm = [ImageSmoke]::FindClass($process.Id, '#32770')
  if ($confirm -ne [IntPtr]::Zero -and [ImageSmoke]::Content($confirm).Contains($webUrl)) { break }
  Start-Sleep -Milliseconds 20
 }
 if ($confirm -eq [IntPtr]::Zero -or -not [ImageSmoke]::Content($confirm).Contains($webUrl)) { throw 'Image confirmation did not show the exact URL' }
 [void][ImageSmoke]::ShowWindow($confirm, 0)
 $choice = if ($accept) { 1 } else { 2 }
 [void][ImageSmoke]::PostMessageW($confirm, 0x111, [IntPtr]$choice, [IntPtr]::Zero)
 for ($i = 0; $i -lt 200 -and [ImageSmoke]::IsWindow($confirm); $i++) { Start-Sleep -Milliseconds 20 }
 if ([ImageSmoke]::IsWindow($confirm)) { throw 'Image confirmation did not close' }
}
try {
 [IO.File]::WriteAllBytes((Join-Path $feed 'body'), [IO.File]::ReadAllBytes((Join-Path $assets 'halves.png')))
 $serverProcess = Start-Process -FilePath $Server -ArgumentList ('"' + $feed + '"') -WindowStyle Hidden -PassThru
 for ($i = 0; $i -lt 200 -and -not (Test-Path -LiteralPath (Join-Path $feed 'port')); $i++) { Start-Sleep -Milliseconds 20 }
 $port = [IO.File]::ReadAllText((Join-Path $feed 'port')).Trim()
 $webUrl = "http://127.0.0.1:$port/image?raw=1&token=a%2Fb"
 [IO.File]::WriteAllText($document, [IO.File]::ReadAllText($document).Replace('https://example.com/logo.png', $webUrl), $utf8)
 # Off by default: no image is read, every image is its plain placeholder.
 StartReader ''
 if ((Query 81) -ne 0 -or (Query 83) -ne 0) { throw "Images drawn or read while off: runs=$(Query 81) bytes=$(Query 83)" }
 $text = Copied
 foreach ($expected in @('[图片: 红蓝]', '[图片: 旋转]', '[图片: 网络 · 点击加载网络图片]', '[图片: 缺失]')) {
  if (-not $text.Contains($expected)) { throw "Placeholder '$expected' missing while images are off: $text" }
 }
 if ((Requests) -ne 0) { throw 'Web image fetched without a click while images are off' }
 Capture ('images-off' + $suffix)
 StopReader

 # On: local raster files are drawn; the rest say why they are not.
 StartReader "images=true`n"
 if ((Query 81) -ne 5) { throw "Expected 5 drawn images, layout has $(Query 81)" }
 $text = Copied
 foreach ($expected in @('红蓝', '旋转', 'build', 'docs', '宽图', '[图片: 网络 · 点击加载网络图片]', '[图片: 缺失 · 找不到文件]', '[图片: 坏文件 · 无法读取]', '[图片: 矢量 · 不支持的格式]')) {
  if (-not $text.Contains($expected)) { throw "Copy text lacks '$expected': $text" }
 }
 if ($haveJunction -and -not $text.Contains('[图片: 链接 · 不读取链接、网络或设备路径]')) { throw "An image behind a junction was not refused: $text" }
 if ($text.Contains('[图片: 红蓝')) { throw 'A drawn image still shows its placeholder' }
 Capture ('images-on' + $suffix)
 if ((Query 84) -ne 5) { throw "Expected 5 images on screen, painted $(Query 84)" }
 $left = Query 80; $body = Query 55

 # <img width="160"> in a centred paragraph: 160 CSS px, aspect kept, centred,
 # transparent pixels show the page.
 $halves = Painted 0
 Near $halves.Width (Scale 160) 2 'Centred image width'
 Near $halves.Height (Scale 80) 2 'Centred image height'
 Near (($halves.Left + $halves.Right) / 2) ($left + $body / 2) 3 'Centred image centre'
 $y = $halves.Top + [int]($halves.Height * 0.4)
 $red = Pixel ($halves.Left + [int]($halves.Width / 4)) $y
 $blue = Pixel ($halves.Right - [int]($halves.Width / 4)) $y
 if ($red[0] -lt 180 -or $red[2] -gt 80) { throw "Left half is not red: $red" }
 if ($blue[2] -lt 180 -or $blue[0] -gt 80) { throw "Right half is not blue: $blue" }
 $band = Pixel ($halves.Left + [int]($halves.Width / 2)) ($halves.Top + [int]($halves.Height * 0.93))
 $page = Pixel ($halves.Left - (Scale 8)) ($halves.Top + [int]($halves.Height * 0.93))
 if (($band -join ',') -ne ($page -join ',')) { throw "Transparent pixels are not the page: $band vs $page" }

 # EXIF orientation 6: stored 40x20, shown 20x40 with the green half on top.
 $rotated = Painted 1
 Near $rotated.Width (Scale 20) 2 'Rotated image width'
 Near $rotated.Height (Scale 40) 2 'Rotated image height'
 $top = Pixel ($rotated.Left + [int]($rotated.Width / 2)) ($rotated.Top + [int]($rotated.Height / 4))
 $bottom = Pixel ($rotated.Left + [int]($rotated.Width / 2)) ($rotated.Bottom - [int]($rotated.Height / 4))
 if ($top[1] -lt 120 -or $top[0] -gt 100) { throw "Rotated image top is not green: $top" }
 if ($bottom[0] -lt 215 -or $bottom[1] -lt 215 -or $bottom[2] -lt 215) { throw "Rotated image bottom is not white: $bottom" }

 # Badges share one line; only the linked one is a link.
 $build = Painted 2; $docs = Painted 3
 Near $build.Width (Scale 90) 2 'Badge width'
 if ($build.Top -ne $docs.Top -or $docs.Left -le $build.Right) { throw 'Badges are not side by side on one line' }
 if ((Query 89 (Point ($build.Left + 4) ($build.Top + 4))) -ne 1) { throw 'The linked badge is not a link' }
 if ((Query 89 (Point ($docs.Left + 4) ($docs.Top + 4))) -ne 2) { throw 'The plain badge is not part of its row' }

 # A wide photo shrinks to the reading column and keeps its aspect.
 $wide = Painted 4
 if ($wide.Left -lt $left -or $wide.Right -gt $left + $body) { throw "Wide image overflows the column: $($wide.Left)-$($wide.Right) in $left-$($left + $body)" }
 Near $wide.Width $body 2 'Wide image width'
 Near $wide.Height ([int]($body / 8)) 2 'Wide image height'

 # Narrower window: images refit, the page stays put. GDI handles are counted after
 # the first paint (fonts and one display copy per image) and must stay flat.
 $gdiBefore = StableGdi
 for ($i = 0; $i -lt 6; $i++) {
  [void][ImageSmoke]::SetWindowPos($window, [IntPtr]::Zero, 0, 0, (Scale (720 + 60 * ($i % 3))), (Scale 820), 0x16); Idle; Capture
 }
 Capture ('images-narrow' + $suffix)
 $narrow = Painted 4
 Near $narrow.Width (Query 55) 2 'Refitted wide image width'
 [void][ImageSmoke]::SetWindowPos($window, [IntPtr]::Zero, 0, 0, (Scale 1080), (Scale 820), 0x16); Idle
 Send 0x100 0x23 0; Idle; Capture
 Send 0x100 0x24 0; Idle; Capture
 if ((Query 84) -lt 4) { throw "Images not drawn again after scrolling back: $(Query 84)" }
 $gdiAfter = StableGdi
 if ($gdiAfter -gt $gdiBefore + 2) { throw "Images leaked GDI handles: $gdiBefore -> $gdiAfter" }
 $bytes = Query 83
 if ($bytes -le 0) { throw 'No decoded image bytes are held' }
 if ((Query 90) -ne 5) { throw "Expected one display bitmap per drawn image, found $(Query 90)" }

 # Settings → Reading → Show images, turned off: images and their memory go at once.
 # Posted: the dialog runs its own modal loop.
 [void][ImageSmoke]::PostMessageW($window, 0x111, [IntPtr]414, [IntPtr]::Zero)
 $dialog = [IntPtr]::Zero
 for ($i = 0; $i -lt 200 -and $dialog -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 25; $dialog = [ImageSmoke]::FindClass($process.Id, '#32770') }
 if ($dialog -eq [IntPtr]::Zero) { throw 'Settings did not open' }
 for ($i = 0; $i -lt 500 -and [ImageSmoke]::GetDlgItem($dialog, 1105) -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 20 }
 # The modal loop shows the dialog on its first idle; hide it until it stays hidden.
 for ($quiet = 0; $quiet -lt 5) {
  if ([ImageSmoke]::IsWindowVisible($dialog)) { [void][ImageSmoke]::ShowWindow($dialog, 0); $quiet = 0 } else { $quiet++ }
  Start-Sleep -Milliseconds 30
 }
 $box = [ImageSmoke]::GetDlgItem($dialog, 1105)
 if ([ImageSmoke]::SendMessageW($box, 0xf0, [IntPtr]::Zero, [IntPtr]::Zero).ToInt64() -ne 1) { throw 'Show images is not checked in Settings' }
 [void][ImageSmoke]::PostMessageW($box, 0xf5, [IntPtr]::Zero, [IntPtr]::Zero)
 Start-Sleep -Milliseconds 80
 [void][ImageSmoke]::PostMessageW($dialog, 0x111, [IntPtr]1, [IntPtr]::Zero)
 for ($i = 0; $i -lt 200 -and [ImageSmoke]::IsWindow($dialog); $i++) { Start-Sleep -Milliseconds 20 }
 if ([ImageSmoke]::IsWindow($dialog)) { throw 'Settings did not close' }
 Idle
 Capture
 if ((Query 81) -ne 0 -or (Query 83) -ne 0) { throw "Turning images off kept them: runs=$(Query 81) bytes=$(Query 83)" }
 if ((Get-Content -LiteralPath $ini -Raw) -notmatch '(?m)^images=false\r?$') { throw 'Show images off not saved' }
 # Their display copies (one GDI bitmap each) are released too.
 if ((Query 90) -ne 0) { throw "Turning images off kept $(Query 90) display bitmaps" }
 $gdiOff = StableGdi
 if ((Requests) -ne 0) { throw 'Enabling local images, resizing or scrolling fetched a web image' }
 StopReader

 # Manual loads work with images off. Duplicate URLs coalesce; the enclosing
 # browser link becomes clickable only after the image has been loaded.
 [IO.File]::WriteAllText($document, "# Manual images`n`n[![First]($webUrl)](https://example.com/not-opened)`n`n![Same]($webUrl)`n`n<img src=`"http://127.0.0.1:$port/second`" alt=`"Second`" width=`"80`">", $utf8)
 StartReader ''
 Capture ('web-before' + $suffix)
 LoadWeb $false; Idle
 if ((Requests) -ne 0 -or (Query 81) -ne 0) { throw 'Cancelling confirmation downloaded an image' }
 LoadWeb $true; Idle; Capture ('web-loaded' + $suffix)
 if ((Requests) -ne 1 -or (Query 81) -ne 2) { throw "Manual load did not coalesce only the requested URL: requests=$(Requests), runs=$(Query 81)" }
 $loaded = Painted 0
 if ((Query 89 (Point ($loaded.Left + 4) ($loaded.Top + 4))) -ne 1) { throw 'Loaded remote image lost its enclosing link' }
 # Saving unrelated preferences must not erase manual images while local images
 # remain off (or leave layout pointing at an empty cache).
 [void][ImageSmoke]::PostMessageW($window, 0x111, [IntPtr]414, [IntPtr]::Zero)
 $dialog = [IntPtr]::Zero
 for ($i = 0; $i -lt 200; $i++) {
  $dialog = [ImageSmoke]::FindClass($process.Id, '#32770')
  if ($dialog -ne [IntPtr]::Zero -and [ImageSmoke]::GetDlgItem($dialog, 1105) -ne [IntPtr]::Zero) { break }
  Start-Sleep -Milliseconds 20
 }
 if ($dialog -eq [IntPtr]::Zero) { throw 'Settings did not open for manual-cache test' }
 [void][ImageSmoke]::ShowWindow($dialog, 0)
 [void][ImageSmoke]::PostMessageW($dialog, 0x111, [IntPtr]1, [IntPtr]::Zero)
 for ($i = 0; $i -lt 200 -and [ImageSmoke]::IsWindow($dialog); $i++) { Start-Sleep -Milliseconds 20 }
 if ([ImageSmoke]::IsWindow($dialog)) { throw 'Settings did not close for manual-cache test' }
 Idle; Capture
 if ((Query 84) -ne 2 -or (Query 83) -le 0 -or (Requests) -ne 1) { throw 'Unrelated settings erased manually loaded images' }
 Send 0x111 205 0; Idle; Capture
 if ((Query 92) -ne 0) { throw 'Source view exposes a remote load action' }
 Send 0x111 205 0; Idle
 if ((Requests) -ne 1) { throw 'View toggle re-downloaded a cached image' }
 Send 0x111 104 0; Idle
 if ((Query 81) -ne 0 -or (Requests) -ne 1) { throw 'Reload retained remote permission or pixels' }
 [IO.File]::WriteAllText((Join-Path $feed 'status'), '404 Not Found')
 LoadWeb $true; Idle
 if (-not (Copied).Contains('下载失败，点击重试')) { throw 'Download failure has no retry action' }
 [IO.File]::WriteAllText((Join-Path $feed 'status'), '200 OK')
 LoadWeb $true; Idle
 if ((Requests) -ne 3 -or (Query 81) -ne 2) { throw 'Manual retry failed' }
 # F5 during a delayed request discards its eventual result, without auto-retry.
 Send 0x111 104 0; Idle
 [IO.File]::WriteAllText((Join-Path $feed 'delay'), '1500')
 LoadWeb $true
 for ($i = 0; $i -lt 200 -and (Requests) -lt 4; $i++) { Start-Sleep -Milliseconds 10 }
 Send 0x111 104 0; Idle
 Start-Sleep -Milliseconds 1700; Idle
 if ((Query 81) -ne 0 -or (Requests) -ne 4) { throw 'An obsolete download appeared after F5' }
 [IO.File]::WriteAllText((Join-Path $feed 'delay'), '0')
 StopReader
 StartReader "images=true`n"
 if ((Query 81) -ne 0 -or (Requests) -ne 4) { throw 'Restart retained remote permission' }
 StopReader

 $metrics = [ordered]@{
  result = 'PASS'; theme = $Theme; author = 'lianzy'; dpi = $dpi
  off_by_default_reads_nothing = $true; drawn_images = 5; placeholders_with_reasons = 4; junction_refused = $haveJunction
  html_width_attribute_and_centre = $true; transparency_shows_page = $true; exif_orientation_6 = $true
  badges_one_line_linked_hit = $true; wide_image_fits_column = $true; refit_on_resize = $true
  decoded_bytes = $bytes; gdi_before = $gdiBefore; gdi_after = $gdiAfter; gdi_images_off = $gdiOff; turning_off_frees_images = $true
  screenshots = @("images-off$suffix.png", "images-on$suffix.png", "images-narrow$suffix.png", "web-before$suffix.png", "web-loaded$suffix.png")
  manual_web_load_cancel_retry_cache_reload = $true; web_requests = (Requests)
  limitations = 'Hidden window, generated fixtures on the local disk; OneDrive placeholders, removable and network drives are covered by unit-level rules only.'
  exe_bytes = (Get-Item -LiteralPath $Exe).Length
 }
 $metrics | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Output 'image.json') -Encoding utf8
 $metrics | ConvertTo-Json
} finally {
 try { StopReader } finally {
  if ($frame) { $frame.Dispose() }
  if ($createdIni) { Remove-Item -LiteralPath $ini -ErrorAction SilentlyContinue }
  if (Test-Path -LiteralPath $junction) { [IO.Directory]::Delete($junction) }
  if ($serverProcess -and -not $serverProcess.HasExited) {
   [IO.File]::WriteAllText((Join-Path $feed 'stop'), '')
   if (-not $serverProcess.WaitForExit(5000)) { $serverProcess.Kill() }
  }
  $env:FEATHERMD_TEST_SNAPSHOT = $oldSnapshot
  if ($oldDpi -ne [IntPtr]::Zero) { [void][ImageSmoke]::SetThreadDpiAwarenessContext($oldDpi) }
 }
}
