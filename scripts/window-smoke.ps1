param([string]$Exe = "$PSScriptRoot\..\target\release\feathermd.exe", [string]$Output = "$PSScriptRoot\..\test-artifacts\window")
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Reflection;
using System.Runtime.InteropServices;
[assembly: AssemblyCompany("lianzy")]
[assembly: AssemblyCopyright("Copyright lianzy")]
public static class WindowSmoke {
 public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
 public struct Point { public int X, Y; }
 public struct Rect { public int Left, Top, Right, Bottom; }
 public struct MinMaxInfo { public Point Reserved, MaxSize, MaxPosition, MinTrackSize, MaxTrackSize; }
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr data);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr hwnd, System.Text.StringBuilder text, int count);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint id);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")] public static extern IntPtr SendText(IntPtr hwnd, uint msg, IntPtr wp, string text);
 [DllImport("user32.dll", EntryPoint="SendMessageW")] public static extern IntPtr SendMinMax(IntPtr hwnd, uint msg, IntPtr wp, ref MinMaxInfo info);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hwnd, out Rect rect);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
 [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hwnd, ref Point point);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int width, int height, uint flags);
 [DllImport("user32.dll")] public static extern IntPtr GetMenu(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtrW(IntPtr hwnd, int index);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
 public static uint Style(IntPtr hwnd) { return unchecked((uint)GetWindowLongPtrW(hwnd, -16).ToInt64()); }
 public static IntPtr Position(int x, int y) { return new IntPtr(((long)(ushort)y << 16) | (ushort)x); }
 public static IntPtr Wheel(int delta) { return new IntPtr((long)(ushort)(short)delta << 16); }
 public static IntPtr Find(uint pid) {
  IntPtr found = IntPtr.Zero;
  EnumWindows((h,d) => {
   uint p; GetWindowThreadProcessId(h, out p);
   var cls = new System.Text.StringBuilder(128); GetClassNameW(h, cls, 128);
   if (p == pid && cls.ToString() == "FeatherMD.Native.Reader") { found = h; return false; }
   return true;
  }, IntPtr.Zero);
  return found;
 }
}
"@

New-Item -ItemType Directory -Path $Output -Force | Out-Null
$Output = (Resolve-Path -LiteralPath $Output).Path
$Exe = (Resolve-Path -LiteralPath $Exe).Path
$document = Join-Path $Output 'resize-long-document.md'
$headingCount = 1200
$lines = [string[]](0..($headingCount - 1) | ForEach-Object {
 '# Heading {0:D5} by lianzy' -f $_
 ''
 'bounded native window rendering ' * 48
 ''
})
[IO.File]::WriteAllLines($document, $lines, [Text.UTF8Encoding]::new($false))
$ini = Join-Path (Split-Path $Exe) 'FeatherMD.ini'
if (Test-Path -LiteralPath $ini) { throw 'Refusing to overwrite existing portable settings' }
$oldSnapshot = $env:FEATHERMD_TEST_SNAPSHOT
$oldDpiContext = [IntPtr]::Zero
$process = $null
$window = [IntPtr]::Zero
$createdIni = $false
$requiresRows = $false
$dpi = 96
$hitTests = 0
function Query([int]$id, [int]$index = 0) { [WindowSmoke]::SendMessageW($window, 0x802a, [IntPtr]$id, [IntPtr]$index).ToInt64() }
function Scale([int]$value) { [int][Math]::Floor($value * $dpi / 96) }
function AssertHidden {
 if ([WindowSmoke]::IsWindowVisible($window)) { throw 'Reader became visible during the hidden-window test' }
}
function Idle {
 for ($i = 0; $i -lt 500; $i++) {
  if ($process.HasExited) { throw 'Reader exited before verification completed' }
  if ((Query 11) -eq 0 -and (Query 0) -gt 0 -and (Query 1) -gt 0 -and (-not $requiresRows -or (Query 2) -gt 0)) { AssertHidden; return }
  Start-Sleep -Milliseconds 10
 }
 throw 'Window layout timed out'
}
function StartReader([string]$path = '') {
 $script:requiresRows = [bool]$path
 $script:window = [IntPtr]::Zero
 if ($path) {
  $script:process = Start-Process -FilePath $Exe -ArgumentList ('"' + $path + '"') -WindowStyle Hidden -PassThru
 } else {
  $script:process = Start-Process -FilePath $Exe -WindowStyle Hidden -PassThru
 }
 for ($i = 0; $i -lt 100; $i++) {
  Start-Sleep -Milliseconds 30
  $script:window = [WindowSmoke]::Find($process.Id)
  if ($window -ne [IntPtr]::Zero) { break }
 }
 if ($window -eq [IntPtr]::Zero) { throw 'Native window not created' }
 Idle
 $script:dpi = [int](Query 3)
}
function StopReader {
 if ($process -and -not $process.HasExited) {
  [void][WindowSmoke]::PostMessageW($window, 0x10, [IntPtr]::Zero, [IntPtr]::Zero)
  if (-not $process.WaitForExit(5000)) { $process.Kill(); throw 'Reader failed to exit cleanly' }
 }
}
function WaitOutline {
 for ($i = 0; $i -lt 1000; $i++) {
  if ($process.HasExited) { throw 'Reader exited while building its outline' }
  if ((Query 33) -eq 0 -and (Query 34) -eq $headingCount) { return }
  Start-Sleep -Milliseconds 10
 }
 throw "Outline did not load $headingCount headings: count=$(Query 34), loading=$(Query 33)"
}
function Click([int]$x, [int]$y) {
 [void][WindowSmoke]::SendMessageW($window, 0x201, [IntPtr]1, [WindowSmoke]::Position($x, $y))
 [void][WindowSmoke]::SendMessageW($window, 0x202, [IntPtr]::Zero, [WindowSmoke]::Position($x, $y))
 Idle
}
function Capture([string]$name) {
 AssertHidden
 if ((Query 4) -ne 1) { throw "Internal capture failed: $(Query 0)x$(Query 1)" }
 $bitmap = [Drawing.Bitmap]::new($env:FEATHERMD_TEST_SNAPSHOT)
 try {
  if ($bitmap.Width -ne (Query 0) -or $bitmap.Height -ne (Query 1)) { throw 'Capture dimensions differ from the client area' }
  $bitmap.Save((Join-Path $Output ($name + '.png')), [Drawing.Imaging.ImageFormat]::Png)
 } finally { $bitmap.Dispose() }
}
function AssertClient([int]$width = 0, [int]$height = 0) {
 $client = [WindowSmoke+Rect]::new()
 $outer = [WindowSmoke+Rect]::new()
 $origin = [WindowSmoke+Point]::new()
 if (-not [WindowSmoke]::GetClientRect($window, [ref]$client) -or -not [WindowSmoke]::GetWindowRect($window, [ref]$outer) -or -not [WindowSmoke]::ClientToScreen($window, [ref]$origin)) { throw 'Cannot read window geometry' }
 if ($client.Right -ne ($outer.Right - $outer.Left) -or $client.Bottom -ne ($outer.Bottom - $outer.Top) -or $origin.X -ne $outer.Left -or $origin.Y -ne $outer.Top) { throw 'Native non-client space remains around the custom client surface' }
 if ((Query 0) -ne $client.Right -or (Query 1) -ne $client.Bottom) { throw 'Application dimensions differ from the actual client rectangle' }
 if ($width -gt 0 -and ($client.Right -ne $width -or $client.Bottom -ne $height)) { throw "Resize mismatch: $($client.Right)x$($client.Bottom), expected ${width}x$height" }
 AssertHidden
}
function AssertChrome {
 $style = [WindowSmoke]::Style($window)
 if (($style -band 0x00c00000) -ne 0) { throw ('Native WS_CAPTION remains enabled: style=0x{0:X8}' -f $style) }
 if (($style -band 0x00300000) -ne 0) { throw 'Native scrollbar styles remain enabled' }
 if ([WindowSmoke]::GetMenu($window) -ne [IntPtr]::Zero) { throw 'A native menu is still attached' }
 AssertClient
}
function Hit([string]$name, [int]$x, [int]$y, [int]$expected) {
 $point = [WindowSmoke+Point]::new()
 $point.X = $x; $point.Y = $y
 if (-not [WindowSmoke]::ClientToScreen($window, [ref]$point)) { throw 'Cannot convert hit-test coordinates' }
 $actual = [WindowSmoke]::SendMessageW($window, 0x84, [IntPtr]::Zero, [WindowSmoke]::Position($point.X, $point.Y)).ToInt64()
 if ($actual -ne $expected) { throw "Hit-test $name returned $actual; expected $expected" }
 $script:hitTests++
}
function CheckHitTests {
 $width = [int](Query 0); $height = [int](Query 1)
 $centerX = [int][Math]::Floor($width / 2); $centerY = [int][Math]::Floor($height / 2)
 Hit 'title blank' (Scale 200) (Scale 24) 2
 Hit 'brand is draggable, not an outline button' (Scale 22) (Scale 24) 2
 for ($i = 0; $i -lt 3; $i++) {
  Hit "window button $i" ($width - (Scale 132) - 1 + (Scale ($i * 44 + 22))) (Scale 24) 1
 }
 Hit 'left' 1 $centerY 10
 Hit 'right' ($width - 2) $centerY 11
 Hit 'top' $centerX 1 12
 Hit 'bottom' $centerX ($height - 2) 15
 Hit 'top left' 1 1 13
 Hit 'top right' ($width - 2) 1 14
 Hit 'bottom left' 1 ($height - 2) 16
 Hit 'bottom right' ($width - 2) ($height - 2) 17
}
try {
 [IO.File]::WriteAllText($ini, "theme=light`nfont_size=18`nwrap=true`n", [Text.UTF8Encoding]::new($false))
 $createdIni = $true
 $env:FEATHERMD_TEST_SNAPSHOT = Join-Path $Output 'viewport.bmp'
 # Read physical coordinates without changing the target's DPI or desktop focus.
 $oldDpiContext = [WindowSmoke]::SetThreadDpiAwarenessContext([IntPtr](-4))
 if ($oldDpiContext -eq [IntPtr]::Zero) { throw 'Cannot select physical DPI coordinates for this test thread' }
 StartReader $document
 WaitOutline
 if ((Query 32) -ne 1 -or (Query 35) -ne (Scale 232)) { throw 'Outline was not visible by default at its wide-window width' }
 Capture 'initial-light'
 AssertChrome
 CheckHitTests
 $info = [WindowSmoke+MinMaxInfo]::new()
 [void][WindowSmoke]::SendMinMax($window, 0x24, [IntPtr]::Zero, [ref]$info)
 $minWidth = Scale 640; $minHeight = Scale 480
 if ($info.MinTrackSize.X -ne $minWidth -or $info.MinTrackSize.Y -ne $minHeight) { throw "Minimum track size mismatch: $($info.MinTrackSize.X)x$($info.MinTrackSize.Y) at $dpi DPI" }

 # The headings are separated by over 1 KiB of body text, so a click must seek
 # to the selected heading's source range rather than merely changing selection.
 $headingIndex = 5
 $headingOffset = Query 37 $headingIndex
 if ($headingOffset -le $headingIndex * 1KB) { throw 'Outline fixture headings are not spaced far enough apart' }
 Click (Scale 40) (Scale (104 + 30 * $headingIndex + 15))
 if ((Query 9) -ne $headingOffset) { throw "Outline click did not seek to heading $headingIndex at byte $headingOffset; got $(Query 9)" }
 Capture 'outline-click'
 $bodyBeforeWheel = Query 9; $wrappedBeforeWheel = Query 14; $outlineBeforeWheel = Query 36
 $wheelPoint = [WindowSmoke+Point]::new()
 $wheelPoint.X = Scale 40; $wheelPoint.Y = Scale 160
 if (-not [WindowSmoke]::ClientToScreen($window, [ref]$wheelPoint)) { throw 'Cannot convert outline wheel coordinates' }
 [void][WindowSmoke]::SendMessageW($window, 0x20a, [WindowSmoke]::Wheel(-120), [WindowSmoke]::Position($wheelPoint.X, $wheelPoint.Y))
 Idle
 $outlineAfterWheel = Query 36
 if ($outlineAfterWheel -le $outlineBeforeWheel -or (Query 9) -ne $bodyBeforeWheel -or (Query 14) -ne $wrappedBeforeWheel) { throw 'Outline wheel did not scroll independently of the document' }
 Capture 'outline-scrolled'
 Click ((Query 35) - (Scale 28)) (Scale 76)
 Idle
 if ((Query 32) -ne 0 -or (Query 35) -ne 0 -or (Query 9) -ne $bodyBeforeWheel) { throw 'Hiding the outline lost the reading position or retained sidebar space' }
 Capture 'outline-hidden'
 if ([IO.File]::ReadAllText($ini) -notmatch '(?m)^outline=false\r?$') { throw 'Hidden outline setting was not saved to the portable INI' }
 StopReader
 StartReader $document
 if ((Query 32) -ne 0 -or (Query 35) -ne 0) { throw 'Hidden outline preference did not survive reopening the reader' }
 Capture 'outline-persisted-hidden'
 Click (Scale 22) (Scale 78)
 WaitOutline
 if ((Query 32) -ne 1 -or (Query 35) -ne (Scale 232)) { throw 'Reopening the outline did not restore its content and width' }
 if ([IO.File]::ReadAllText($ini) -notmatch '(?m)^outline=true\r?$') { throw 'Reopened outline setting was not persisted' }
 Capture 'outline-reopened'

 [void][WindowSmoke]::SendMessageW($window, 0x111, [IntPtr]103, [IntPtr]::Zero)
 $edit = [IntPtr](Query 7)
 if ($edit -eq [IntPtr]::Zero) { throw 'Go-to edit control was not created' }
 [void][WindowSmoke]::SendText($edit, 0x0c, [IntPtr]::Zero, '50')
 [void][WindowSmoke]::PostMessageW($edit, 0x100, [IntPtr]13, [IntPtr]::Zero)
 $size = Query 10
 if ($size -lt 1MB) { throw 'Resize fixture is too small' }
 for ($i = 0; $i -lt 500; $i++) {
  if ((Query 11) -eq 0 -and [Math]::Abs((Query 9) / $size - 0.5) -lt 0.01) { break }
  Start-Sleep -Milliseconds 10
 }
 if ($i -eq 500) { throw 'Go-to did not reach the middle of the fixture' }
 $beforeResize = Query 9
 # SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE; no visibility flags.
 if (-not [WindowSmoke]::SetWindowPos($window, [IntPtr]::Zero, 0, 0, $minWidth, $minHeight, 0x16)) { throw 'Native resize failed' }
 Idle
 AssertClient $minWidth $minHeight
 if ((Query 35) -ne (Scale 192)) { throw 'The outline did not adapt to its narrow-window width' }
 $afterResize = Query 9
 if ($afterResize -ne $beforeResize) { throw "Resize lost the source position: $beforeResize -> $afterResize" }
 Capture 'narrow-light'
 CheckHitTests
 [void][WindowSmoke]::SendMessageW($window, 0x111, [IntPtr]201, [IntPtr]::Zero)
 Idle
 Capture 'narrow-dark'
 [void][WindowSmoke]::SendMessageW($window, 0x111, [IntPtr]102, [IntPtr]::Zero)
 if (([WindowSmoke]::Style($edit) -band 0x10000000) -eq 0) { throw 'Search did not expand its edit control' }
 Capture 'narrow-search'
 Click ((Query 35) - (Scale 28)) (Scale 76)
 if ((Query 32) -ne 0) { throw 'Cannot collapse outline while searching' }
 $editRect = [WindowSmoke+Rect]::new(); $windowRect = [WindowSmoke+Rect]::new()
 if (-not [WindowSmoke]::GetWindowRect($edit, [ref]$editRect) -or -not [WindowSmoke]::GetWindowRect($window, [ref]$windowRect)) { throw 'Cannot inspect search control geometry' }
 if ($editRect.Left - $windowRect.Left -lt (Scale 48)) { throw 'Search control overlaps the collapsed outline affordance' }
 Capture 'narrow-search-outline-hidden'
 Click (Scale 22) (Scale 78)
 if ((Query 32) -ne 1 -or ([WindowSmoke]::Style($edit) -band 0x10000000) -eq 0) { throw 'Cannot reopen outline while keeping search visible' }
 AssertChrome

 StopReader
 [IO.File]::WriteAllText($ini, "theme=light`nfont_size=18`nwrap=true`n", [Text.UTF8Encoding]::new($false))
 StartReader
 if ((Query 10) -ne 0 -or (Query 2) -ne 0) { throw 'Welcome window unexpectedly loaded a document' }
 Capture 'welcome-light'
 AssertChrome
 CheckHitTests
 $metrics = [ordered]@{
  result = 'PASS'; author = 'lianzy'; company = 'lianzy'; last_modified_by = 'lianzy'
  input = 'hidden windows; native messages and SetWindowPos; no desktop input injection'
  dpi = $dpi; min_client_width = $minWidth; min_client_height = $minHeight
  native_caption_absent = $true; native_scrollbars_absent = $true; native_menu_absent = $true
  hit_tests = $hitTests; fixture_bytes = $size; before_resize_byte = $beforeResize; after_resize_byte = $afterResize
  outline_headings = $headingCount; outline_clicked_byte = $headingOffset; outline_wheel_top = $outlineAfterWheel
  outline_toggle_preserves_position = $true; outline_preference_survives_restart = $true
  screenshots = @('initial-light.png', 'outline-click.png', 'outline-scrolled.png', 'outline-hidden.png', 'outline-persisted-hidden.png', 'outline-reopened.png', 'narrow-light.png', 'narrow-dark.png', 'narrow-search.png', 'narrow-search-outline-hidden.png', 'welcome-light.png')
  limitations = 'Real pointer input, maximize/minimize/restore, monitor changes, snap, and outline scan limits are not exercised by this hidden-window test.'
  exe_bytes = (Get-Item -LiteralPath $Exe).Length
 }
 $metrics | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Output 'window.json') -Encoding utf8
 $metrics | ConvertTo-Json
} finally {
 try { StopReader } finally {
  if ($createdIni) { Remove-Item -LiteralPath $ini -ErrorAction SilentlyContinue }
  $env:FEATHERMD_TEST_SNAPSHOT = $oldSnapshot
  if ($oldDpiContext -ne [IntPtr]::Zero) { [void][WindowSmoke]::SetThreadDpiAwarenessContext($oldDpiContext) }
 }
}
