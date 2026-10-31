param([string]$Exe = "$PSScriptRoot\..\target\release\feathermd.exe", [string]$Output = "$PSScriptRoot\..\test-artifacts\window", [switch]$AllowDesktopUi, [switch]$SecondaryMonitor)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/require-ui-test-host.ps1" -AllowDesktopUi:$AllowDesktopUi -SecondaryMonitor:$SecondaryMonitor -Exe $Exe
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
 public struct MonitorInfo { public int Size; public Rect Monitor, Work; public uint Flags; }
 [DllImport("user32.dll")] public static extern IntPtr MonitorFromWindow(IntPtr hwnd, uint flags);
 [DllImport("user32.dll")] public static extern bool GetMonitorInfoW(IntPtr monitor, ref MonitorInfo info);
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
$defaultFrame = ''
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
# A new reader is 1080x820 logical pixels at its monitor's scale, capped by the
# work area but never below the 640x480 minimum, and placed inside the work area.
function AssertDefaultFrame {
 $info = [WindowSmoke+MonitorInfo]::new()
 $info.Size = [Runtime.InteropServices.Marshal]::SizeOf([type][WindowSmoke+MonitorInfo])
 if (-not [WindowSmoke]::GetMonitorInfoW([WindowSmoke]::MonitorFromWindow($window, 2), [ref]$info)) { throw 'Cannot read the reader monitor' }
 $work = $info.Work
 $width = [Math]::Max([Math]::Min((Scale 1080), $work.Right - $work.Left), (Scale 640))
 $height = [Math]::Max([Math]::Min((Scale 820), $work.Bottom - $work.Top), (Scale 480))
 $outer = [WindowSmoke+Rect]::new()
 if (-not [WindowSmoke]::GetWindowRect($window, [ref]$outer)) { throw 'Cannot read the default window frame' }
 $actual = "$($outer.Right - $outer.Left)x$($outer.Bottom - $outer.Top)"
 if ($actual -ne "${width}x$height") { throw "Default window is $actual at $dpi DPI; expected ${width}x$height in work area $($work.Right - $work.Left)x$($work.Bottom - $work.Top)" }
 if ($outer.Left -lt $work.Left -or $outer.Top -lt $work.Top -or ($width -le ($work.Right - $work.Left) -and $outer.Right -gt $work.Right) -or ($height -le ($work.Bottom - $work.Top) -and $outer.Bottom -gt $work.Bottom)) { throw "Default window $($outer.Left),$($outer.Top),$($outer.Right),$($outer.Bottom) leaves work area $($work.Left),$($work.Top),$($work.Right),$($work.Bottom)" }
 if ((Query 0) -lt (Scale 800)) { throw "Work area too narrow for the wide-outline checks: $(Query 0) px at $dpi DPI" }
 $script:defaultFrame = "$actual in $($work.Right - $work.Left)x$($work.Bottom - $work.Top)"
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
 AssertDefaultFrame
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
 # Hidden windows do not get normal visible-window repaint scheduling.
 # Paint the current overlay before hit-testing its next synthetic click.
 if ((Query 4) -ne 1) { throw 'Pre-click paint failed' }
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
 Hit 'title label' (Scale 40) (Scale 17) 2
 Hit 'brand is draggable, not an outline button' (Scale 22) (Scale 17) 2
 for ($i = 0; $i -lt 3; $i++) {
  Hit "window button $i" ($width - (Scale 132) - 1 + (Scale ($i * 44 + 22))) (Scale 17) 1
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
function HeaderLeft { return ((Query 0) - (Scale 132) - (Scale 148) - 1) }
# Middle of outline row $row (on screen from the sidebar's first listed row).
function OutlineRowY([int]$row) { return [int]((Query 110) + ($row - (Query 36)) * (Query 111) + [Math]::Floor((Query 111) / 2)) }
# Footer preview/source switch: segment centres 26 and 54 px from the left edge.
function FooterY { return ((Query 1) - (Scale 15)) }
function ThemePicker {
 Click ((HeaderLeft) + (Scale 88)) (Scale 17)
 if ((Query 42) -lt 0) { throw 'Theme icon did not open the picker' }
}
function PickTheme([int]$index) {
 ThemePicker
 Click ((HeaderLeft) - (Scale 20)) ((Query 45) + (Scale (38 + 42 * $index + 19)))
 if ((Query 42) -ne -1) { throw 'Theme choice did not close the picker' }
}
function HeaderKey([int]$key) {
 [void][WindowSmoke]::SendMessageW($window, 0x100, [IntPtr]$key, [IntPtr]::Zero)
 Idle
}
function CheckHeaderSwitches {
 $left = HeaderLeft
 if ((Query 49) -ne (Scale 34)) { throw "Header is not 34 logical pixels: $(Query 49)" }
 foreach ($offset in @(16,52,88,124)) { Hit 'header icon' ($left + (Scale $offset)) (Scale 17) 1 }
 foreach ($offset in @(26,54)) { Hit 'footer mode switch' (Scale $offset) (FooterY) 1 }
 Click (Scale 54) (FooterY)
 if ((Query 38) -ne 1) { throw 'Footer source switch failed' }
 Capture 'mode-source'
 Click (Scale 26) (FooterY)
 if ((Query 38) -ne 0) { throw 'Footer preview switch failed' }
 [void][WindowSmoke]::SendMessageW($window, 0x200, [IntPtr]::Zero, [WindowSmoke]::Position((Scale 54), (FooterY)))
 if ((Query 44) -ne 410) { throw 'Source switch did not receive hover' }
 if ((Query 43) -ne 0) { throw 'Tooltip appeared without its delay' }
 Start-Sleep -Milliseconds 600
 if ((Query 43) -ne 1) { throw "Footer switch tooltip did not appear; hover=$(Query 44)" }
 Capture 'mode-source-tooltip'
 [void][WindowSmoke]::SendMessageW($window, 0x2a3, [IntPtr]::Zero, [IntPtr]::Zero)
 [void][WindowSmoke]::SendMessageW($window, 0x200, [IntPtr]::Zero, [WindowSmoke]::Position(($left + (Scale 16)), (Scale 17)))
 Start-Sleep -Milliseconds 600
 if ((Query 43) -ne 1 -or (Query 44) -ne 101) { throw "Header icon tooltip did not appear; hover=$(Query 44)" }
 Capture 'header-open-tooltip'
 [void][WindowSmoke]::SendMessageW($window, 0x2a3, [IntPtr]::Zero, [IntPtr]::Zero)
 if ((Query 43) -ne 0) { throw 'Tooltip did not hide on mouse leave' }
 ThemePicker
 Capture 'theme-picker-light'
 # Panel title is inert, not a click-through into the document.
 Click ($left - (Scale 20)) (Scale 60)
 if ((Query 42) -lt 0) { throw 'Panel whitespace unexpectedly closed the picker' }
 HeaderKey 27
 if ((Query 42) -ne -1 -or (Query 39) -ne 1) { throw 'Escape did not cancel the theme picker' }
 ThemePicker
 $before = Query 9
 Click (Scale 300) (Scale 330)
 if ((Query 42) -ne -1 -or (Query 9) -ne $before -or (Query 39) -ne 1) { throw 'Outside click did not cancel without activating content' }
 ThemePicker
 Click ($left + (Scale 88)) (Scale 17)
 if ((Query 42) -ne -1) { throw 'Clicking theme icon again did not close the picker' }
 ThemePicker
 HeaderKey 40
 if ((Query 42) -ne 1) { throw 'Theme picker Down did not focus dark' }
 HeaderKey 13
 if ((Query 39) -ne 2 -or (Query 40) -ne 1 -or (Query 42) -ne -1) { throw 'Theme picker keyboard selection failed' }
 ThemePicker
 Capture 'theme-picker-dark'
 [void][WindowSmoke]::SendMessageW($window, 8, [IntPtr]::Zero, [IntPtr]::Zero)
 if ((Query 42) -ne -1) { throw 'Losing focus did not dismiss theme picker' }
 [void][WindowSmoke]::SendMessageW($window, 0x1a, [IntPtr]::Zero, [IntPtr]::Zero)
 if ((Query 39) -ne 2 -or (Query 40) -ne 1) { throw 'System broadcast overrode manual dark mode' }
 PickTheme 0
 if ((Query 39) -ne 1 -or (Query 40) -ne 0) { throw 'Top light choice failed' }
 PickTheme 1
 if ((Query 39) -ne 2 -or (Query 40) -ne 1) { throw 'Top dark choice failed' }
 PickTheme 2
 if ((Query 39) -ne 0 -or (Query 40) -ne (Query 41)) { throw 'System theme did not resolve to the OS app theme' }
 [void][WindowSmoke]::SendMessageW($window, 0x31a, [IntPtr]::Zero, [IntPtr]::Zero)
 if ((Query 40) -ne (Query 41)) { throw 'System theme refresh failed' }
 Capture 'header-follow-system'
 PickTheme 0
 Click ($left + (Scale 52)) (Scale 17)
 if ((Query 7) -eq 0) { throw 'Search icon did not open search' }
 HeaderKey 27
}

try {
 [IO.File]::WriteAllText($ini, "theme=light`nfont_size=18`nwrap=true`nlanguage=zh-CN`n", [Text.UTF8Encoding]::new($false))
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
 CheckHeaderSwitches
 $info = [WindowSmoke+MinMaxInfo]::new()
 [void][WindowSmoke]::SendMinMax($window, 0x24, [IntPtr]::Zero, [ref]$info)
 $minWidth = Scale 640; $minHeight = Scale 480
 if ($info.MinTrackSize.X -ne $minWidth -or $info.MinTrackSize.Y -ne $minHeight) { throw "Minimum track size mismatch: $($info.MinTrackSize.X)x$($info.MinTrackSize.Y) at $dpi DPI" }

 # The headings are separated by over 1 KiB of body text, so a click must seek
 # to the selected heading's source range rather than merely changing selection.
 $headingIndex = 5
 $headingOffset = Query 37 $headingIndex
 if ($headingOffset -le $headingIndex * 1KB) { throw 'Outline fixture headings are not spaced far enough apart' }
 Click (Scale 40) (OutlineRowY $headingIndex)
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
 Click ((Query 35) - (Scale 28)) (Scale 62)
 Idle
 if ((Query 32) -ne 0 -or (Query 35) -ne 0 -or (Query 9) -ne $bodyBeforeWheel) { throw 'Hiding the outline lost the reading position or retained sidebar space' }
 Capture 'outline-hidden'
 if ([IO.File]::ReadAllText($ini) -notmatch '(?m)^outline=false\r?$') { throw 'Hidden outline setting was not saved to the portable INI' }
 StopReader
 StartReader $document
 if ((Query 32) -ne 0 -or (Query 35) -ne 0) { throw 'Hidden outline preference did not survive reopening the reader' }
 Capture 'outline-persisted-hidden'
 Click (Scale 22) (Scale 64)
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
 CheckHeaderSwitches
 [void][WindowSmoke]::SendMessageW($window, 0x111, [IntPtr]201, [IntPtr]::Zero)
 Idle
 Capture 'narrow-dark'
 [void][WindowSmoke]::SendMessageW($window, 0x111, [IntPtr]102, [IntPtr]::Zero)
 if (([WindowSmoke]::Style($edit) -band 0x10000000) -eq 0) { throw 'Search did not expand its edit control' }
 Capture 'narrow-search'
 ThemePicker
 if ((Query 45) -ne (Scale 90)) { throw 'Theme picker overlaps the native search edit' }
 Capture 'narrow-search-theme-picker'
 HeaderKey 27
 Click ((Query 35) - (Scale 28)) (Scale 62)
 if ((Query 32) -ne 0) { throw 'Cannot collapse outline while searching' }
 $editRect = [WindowSmoke+Rect]::new(); $windowRect = [WindowSmoke+Rect]::new()
 if (-not [WindowSmoke]::GetWindowRect($edit, [ref]$editRect) -or -not [WindowSmoke]::GetWindowRect($window, [ref]$windowRect)) { throw 'Cannot inspect search control geometry' }
 if ($editRect.Left - $windowRect.Left -lt (Scale 48)) { throw 'Search control overlaps the collapsed outline affordance' }
 Capture 'narrow-search-outline-hidden'
 Click (Scale 22) (Scale 64)
 if ((Query 32) -ne 1 -or ([WindowSmoke]::Style($edit) -band 0x10000000) -eq 0) { throw 'Cannot reopen outline while keeping search visible' }
 AssertChrome

 # Persist the user's preference, not the resolved system color.
 Click (Scale 54) (FooterY)
 PickTheme 2
 StopReader
 StartReader $document
 if ((Query 38) -ne 1 -or (Query 39) -ne 0 -or (Query 40) -ne (Query 41)) { throw 'Top mode/theme settings did not survive restart' }
 Capture 'header-settings-restored'
 StopReader

 # Short sections fit on one screen, so the top row cannot reach later headings.
 # A click must still move the outline highlight to the heading the user chose.
 $shortDocument = Join-Path $Output 'short-sections.md'
 $shortCount = 8
 [IO.File]::WriteAllLines($shortDocument, [string[]](0..($shortCount - 1) | ForEach-Object { '## Short {0} by lianzy' -f $_; ''; 'brief'; '' }), [Text.UTF8Encoding]::new($false))
 StartReader $shortDocument
 for ($i = 0; $i -lt 500 -and ((Query 33) -ne 0 -or (Query 34) -ne $shortCount); $i++) { Start-Sleep -Milliseconds 10 }
 if ((Query 34) -ne $shortCount) { throw 'Short-section outline did not load' }
 if ((Query 46) -ne 0) { throw "Short-section outline did not start on the first heading: $(Query 46)" }
 foreach ($shortIndex in @(3, 6, 1)) {
  Click (Scale 40) (OutlineRowY $shortIndex)
  if ((Query 46) -ne $shortIndex) { throw "Outline highlight stayed on $(Query 46) after clicking short heading $shortIndex" }
 }
 Capture 'outline-short-click'
 [void][WindowSmoke]::SendMessageW($window, 0x100, [IntPtr]0x24, [IntPtr]::Zero)
 Idle
 if ((Query 46) -ne 0) { throw 'Keyboard navigation did not release the clicked outline highlight' }
 StopReader
 [IO.File]::WriteAllText($ini, "theme=light`nfont_size=18`nwrap=true`nlanguage=zh-CN`n", [Text.UTF8Encoding]::new($false))
 StartReader
 if ((Query 10) -ne 0 -or (Query 2) -ne 0) { throw 'Welcome window unexpectedly loaded a document' }
 if ((Query 35) -ne 0) { throw 'Welcome still reserves an empty sidebar' }
 Capture 'welcome-light'
 PickTheme 1
 Capture 'welcome-dark'
 ThemePicker
 Capture 'welcome-theme-picker'
 HeaderKey 27
 PickTheme 0
 [void][WindowSmoke]::SetWindowPos($window, [IntPtr]::Zero, 0, 0, $minWidth, $minHeight, 0x16)
 Idle
 Capture 'welcome-narrow'
 ThemePicker
 [void][WindowSmoke]::SetWindowPos($window, [IntPtr]::Zero, 0, 0, ($minWidth + 20), $minHeight, 0x16)
 Idle
 if ((Query 42) -ne -1) { throw 'Resizing did not dismiss the theme picker' }
 AssertChrome
 CheckHitTests
 $metrics = [ordered]@{
  result = 'PASS'; author = 'lianzy'; company = 'lianzy'; last_modified_by = 'lianzy'
  input = 'hidden windows; native messages and SetWindowPos; no desktop input injection'
  dpi = $dpi; default_window = $defaultFrame; min_client_width = $minWidth; min_client_height = $minHeight
  native_caption_absent = $true; native_scrollbars_absent = $true; native_menu_absent = $true
  hit_tests = $hitTests; fixture_bytes = $size; before_resize_byte = $beforeResize; after_resize_byte = $afterResize
  outline_headings = $headingCount; outline_clicked_byte = $headingOffset; outline_wheel_top = $outlineAfterWheel
  outline_toggle_preserves_position = $true; outline_short_section_click_highlight = $true; outline_preference_survives_restart = $true
  compact_header_icons_and_tooltips = $true; theme_picker_mouse_keyboard_and_dismissal = $true; welcome_has_no_empty_sidebar = $true
  footer_mode_and_header_theme_switches = $true; system_theme_refresh_and_persistence = $true
  screenshots = @('initial-light.png', 'outline-click.png', 'outline-scrolled.png', 'outline-hidden.png', 'outline-persisted-hidden.png', 'outline-reopened.png', 'narrow-light.png', 'narrow-dark.png', 'narrow-search.png', 'narrow-search-outline-hidden.png', 'welcome-light.png', 'welcome-dark.png', 'welcome-narrow.png', 'theme-picker-light.png', 'theme-picker-dark.png', 'mode-source-tooltip.png', 'header-open-tooltip.png', 'narrow-search-theme-picker.png', 'outline-short-click.png')
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

. "$PSScriptRoot/verify-ui-test-host.ps1" -Output $Output -SecondaryMonitor:$SecondaryMonitor
