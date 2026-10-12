param([string]$Exe = "$PSScriptRoot\..\target\release\feathermd.exe", [string]$Output = "$PSScriptRoot\..\test-artifacts\scrollbar")
$ErrorActionPreference = 'Stop'
Add-Type @"
using System;
using System.Reflection;
using System.Runtime.InteropServices;
[assembly: AssemblyCompany("lianzy")]
[assembly: AssemblyCopyright("Copyright lianzy")]
public static class ScrollbarSmoke {
 public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr data);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr hwnd, System.Text.StringBuilder text, int count);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint id);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
 public static IntPtr Point(int x, int y) { return new IntPtr(((long)(ushort)y << 16) | (ushort)x); }
 public static IntPtr Wheel(int delta) { return new IntPtr((long)(ushort)(short)delta << 16); }
 public static void RoundTrip(IntPtr hwnd, int x, int y, int farY) {
  SendMessageW(hwnd, 0x201, new IntPtr(1), Point(x, y));
  SendMessageW(hwnd, 0x200, new IntPtr(1), Point(x, farY));
  SendMessageW(hwnd, 0x200, new IntPtr(1), Point(x, y));
  SendMessageW(hwnd, 0x202, IntPtr.Zero, Point(x, y));
 }
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
$document = Join-Path $Output 'long-lines.md'
$lines = [string[]](0..1199 | ForEach-Object {
 ('Line {0:D5} by lianzy ' -f $_) + ('abcdefghijklmnopqrstuvwxyz 0123456789 ' * 80)
})
[IO.File]::WriteAllLines($document, $lines, [Text.UTF8Encoding]::new($false))
$ini = Join-Path (Split-Path $Exe) 'FeatherMD.ini'
if (Test-Path -LiteralPath $ini) { throw 'Refusing to overwrite existing portable settings' }
$oldSnapshot = $env:FEATHERMD_TEST_SNAPSHOT
$process = $null
$window = [IntPtr]::Zero
$createdIni = $false
$oldDpiContext = [IntPtr]::Zero
function Query([int]$id) { [ScrollbarSmoke]::SendMessageW($window, 0x802a, [IntPtr]$id, [IntPtr]::Zero).ToInt64() }
function Cursor { ((Query 9) * 1MB) + (Query 14) }
function Idle {
 for ($i = 0; $i -lt 500; $i++) {
  if ($process.HasExited) { throw 'Reader exited during scrollbar regression' }
  if ((Query 11) -eq 0 -and (Query 2) -gt 0) { return }
  Start-Sleep -Milliseconds 10
 }
 throw 'Page load timed out'
}
function Capture([string]$name) {
 if ((Query 4) -ne 1) { throw "Viewport capture failed: size=$(Query 0)x$(Query 1), path=$env:FEATHERMD_TEST_SNAPSHOT" }
 if ($name) { Copy-Item -LiteralPath $env:FEATHERMD_TEST_SNAPSHOT -Destination (Join-Path $Output ($name + '.bmp')) }
}
function Settle { Idle; Capture '' }
function StartReader([string]$path) {
 $script:window = [IntPtr]::Zero
 $script:process = Start-Process -FilePath $Exe -ArgumentList ('"' + $path + '"') -WindowStyle Hidden -PassThru
 for ($i = 0; $i -lt 100; $i++) {
  Start-Sleep -Milliseconds 30
  $script:window = [ScrollbarSmoke]::Find($process.Id)
  if ($window -ne [IntPtr]::Zero) { break }
 }
 if ($window -eq [IntPtr]::Zero) { throw 'Native window not created' }
 Settle
}
function StopReader {
 if ($process -and -not $process.HasExited) {
  [void][ScrollbarSmoke]::PostMessageW($window, 0x10, [IntPtr]::Zero, [IntPtr]::Zero)
  if (-not $process.WaitForExit(5000)) { $process.Kill(); throw 'Reader failed to exit cleanly' }
 }
}
function Mouse([uint32]$message, [int]$x, [int]$y, [int]$buttons = 0) {
 [void][ScrollbarSmoke]::SendMessageW($window, $message, [IntPtr]$buttons, [ScrollbarSmoke]::Point($x, $y))
}
function Home {
 [void][ScrollbarSmoke]::SendMessageW($window, 0x100, [IntPtr]0x24, [IntPtr]::Zero)
 Settle
 if ((Cursor) -ne 0) { throw 'Home did not restore the first row' }
}
function Geometry([bool]$horizontal = $false) {
 $base = if ($horizontal) { 25 } else { 19 }
 $g = [pscustomobject]@{
  Left = [int](Query $base); Top = [int](Query ($base + 1))
  Right = [int](Query ($base + 2)); Bottom = [int](Query ($base + 3))
  ThumbStart = [int](Query ($base + 4)); ThumbEnd = [int](Query ($base + 5))
 }
 if ($g.Left -lt 0 -or $g.Top -lt 0 -or $g.Right -le $g.Left -or $g.Bottom -le $g.Top -or $g.ThumbEnd -le $g.ThumbStart) {
  throw "Missing or invalid custom scrollbar geometry (horizontal=$horizontal); this test requires diagnostics 18-31"
 }
 return $g
}
function Drag([double]$fraction, [bool]$horizontal = $false, [bool]$doubleClick = $false) {
 $g = Geometry $horizontal
 $grab = [int][Math]::Floor(($g.ThumbEnd - $g.ThumbStart) / 2)
 $axisStart = if ($horizontal) { $g.Left } else { $g.Top }
 $axisEnd = if ($horizontal) { $g.Right } else { $g.Bottom }
 $target = $axisStart + [int][Math]::Round(($axisEnd - $axisStart - ($g.ThumbEnd - $g.ThumbStart)) * $fraction) + $grab
 $x = if ($horizontal) { $g.ThumbStart + $grab } else { [int][Math]::Floor(($g.Left + $g.Right) / 2) }
 $y = if ($horizontal) { [int][Math]::Floor(($g.Top + $g.Bottom) / 2) } else { $g.ThumbStart + $grab }
 if ($doubleClick) {
  Mouse 0x201 $x $y 1
  Mouse 0x202 $x $y
  Mouse 0x203 $x $y 1
 } else {
  Mouse 0x201 $x $y 1
 }
 if ($horizontal) { $x = $target } else { $y = $target }
 Mouse 0x200 $x $y 1
 Settle
 # Measure before button-up: movement must update the viewport while dragging.
 $position = if ($horizontal) { Query 18 } else { Query 9 }
 Mouse 0x202 $x $y
 Settle
 return $position
}
try {
 # Match physical coordinates returned by the reader on scaled displays.
 $oldDpiContext = [ScrollbarSmoke]::SetThreadDpiAwarenessContext([IntPtr](-4))
 [IO.File]::WriteAllText($ini, "theme=light`nfont_size=18`nwrap=true`nraw=false`nwide=false`n", [Text.UTF8Encoding]::new($false))
 $createdIni = $true
 $env:FEATHERMD_TEST_SNAPSHOT = Join-Path $Output 'viewport.bmp'
 # Only send native window messages. Never activate the window or inject desktop input.
 StartReader $document
 $size = Query 10
 if ($size -lt 1MB) { throw "Long-line fixture is too small: $size bytes" }
 if ((Cursor) -ne 0) { throw 'Initial viewport did not begin at zero' }
 $middle = Drag 0.5
 if ([Math]::Abs($middle / $size - 0.5) -gt 0.08) { throw "Vertical thumb did not drag to the middle: $middle / $size" }
 Capture 'vertical-middle'
 $bottom = Drag 1.0
 if ($bottom -le $middle -or $bottom / $size -lt 0.95) { throw "Vertical thumb did not reach the end: $bottom / $size" }
 Capture 'vertical-bottom'
 $top = Drag 0.0
 if ($top -ne 0 -or (Cursor) -ne 0) { throw "Vertical thumb did not return to the first row: $top" }

 # With CS_DBLCLKS, a rapid second press becomes WM_LBUTTONDBLCLK.
 # Clicking the thumb and then dragging must still move during that second press.
 $doubleClickMiddle = Drag 0.5 $false $true
 if ([Math]::Abs($doubleClickMiddle / $size - 0.5) -gt 0.08) { throw "Vertical thumb did not drag after WM_LBUTTONDBLCLK: $doubleClickMiddle / $size" }
 Home

 # Queue a seek outside the cache and return immediately, before waiting on I/O.
 for ($roundTrip = 0; $roundTrip -lt 3; $roundTrip++) {
  $g = Geometry
  $x = [int][Math]::Floor(($g.Left + $g.Right) / 2)
  $y = [int][Math]::Floor(($g.ThumbStart + $g.ThumbEnd) / 2)
  [ScrollbarSmoke]::RoundTrip($window, $x, $y, ($g.Bottom - 2))
  Settle
  for ($poll = 0; $poll -lt 20; $poll++) {
   Start-Sleep -Milliseconds 25
   if ((Cursor) -ne 0) { throw 'A stale page reply moved the viewport after a rapid drag back to the top' }
  }
 }

 # Track clicks page in the clicked direction instead of seeking straight there.
 $g = Geometry
 $x = [int][Math]::Floor(($g.Left + $g.Right) / 2)
 $y = $g.Bottom - 2
 Mouse 0x201 $x $y 1
 Mouse 0x202 $x $y
 Settle
 $pageCursor = Cursor
 if ($pageCursor -le 0 -or (Query 9) -ge $size / 10) { throw 'Vertical track click did not page down' }
 $middleAgain = Drag 0.5
 $beforePageUp = Cursor
 $g = Geometry
 $y = $g.Top + 1
 Mouse 0x201 $x $y 1
 Mouse 0x202 $x $y
 Settle
 if ((Cursor) -ge $beforePageUp -or (Query 9) -lt $size / 4) { throw 'Vertical track click did not page up' }

 Home
 [void][ScrollbarSmoke]::SendMessageW($window, 0x20a, [ScrollbarSmoke]::Wheel(-120), [IntPtr]::Zero)
 Settle
 $wheelCursor = Cursor
 if ($wheelCursor -le 0) { throw 'Mouse wheel did not scroll down through wrapped rows' }
 [void][ScrollbarSmoke]::SendMessageW($window, 0x20a, [ScrollbarSmoke]::Wheel(120), [IntPtr]::Zero)
 Settle
 if ((Cursor) -ne 0) { throw 'Mouse wheel did not return to the first row' }

 # Losing capture cancels a drag even when a later move still has MK_LBUTTON.
 $g = Geometry
 $x = [int][Math]::Floor(($g.Left + $g.Right) / 2)
 $y = [int][Math]::Floor(($g.ThumbStart + $g.ThumbEnd) / 2)
 Mouse 0x201 $x $y 1
 [void][ScrollbarSmoke]::SendMessageW($window, 0x215, [IntPtr]::Zero, [IntPtr]::Zero)
 $beforeCancel = Cursor
 Mouse 0x200 $x ($g.Bottom - 2) 1
 Settle
 if ((Cursor) -ne $beforeCancel) { throw 'Vertical drag continued after capture changed' }
 Mouse 0x202 $x ($g.Bottom - 2)

 [void][ScrollbarSmoke]::SendMessageW($window, 0x111, [IntPtr]204, [IntPtr]::Zero)
 Settle
 $horizontalMax = Query 31
 if ($horizontalMax -le 0 -or (Query 18) -ne 0) { throw 'Unwrapped long lines did not expose a horizontal range' }
 $horizontalMiddle = Drag 0.5 $true
 if ([Math]::Abs($horizontalMiddle / $horizontalMax - 0.5) -gt 0.08) { throw "Horizontal thumb did not drag to the middle: $horizontalMiddle / $horizontalMax" }
 Capture 'horizontal-middle'
 $horizontalEnd = Drag 1.0 $true
 if ([Math]::Abs($horizontalEnd - (Query 31)) -gt 1) { throw 'Horizontal thumb did not reach the right edge' }
 $horizontalStart = Drag 0.0 $true
 if ($horizontalStart -ne 0 -or (Query 18) -ne 0) { throw 'Horizontal thumb did not return to the left edge' }
 $g = Geometry $true
 $x = [int][Math]::Floor(($g.ThumbStart + $g.ThumbEnd) / 2)
 $y = [int][Math]::Floor(($g.Top + $g.Bottom) / 2)
 Mouse 0x201 $x $y 1
 [void][ScrollbarSmoke]::SendMessageW($window, 0x215, [IntPtr]::Zero, [IntPtr]::Zero)
 Mouse 0x200 ($g.Right - 2) $y 1
 Settle
 if ((Query 18) -ne 0) { throw 'Horizontal drag continued after capture changed' }
 Mouse 0x202 ($g.Right - 2) $y

 StopReader
 [IO.File]::WriteAllText($ini, "theme=light`nfont_size=18`nwrap=true`n", [Text.UTF8Encoding]::new($false))
 $shortDocument = Join-Path $Output 'short.md'
 [IO.File]::WriteAllText($shortDocument, 'Short document by lianzy.', [Text.UTF8Encoding]::new($false))
 StartReader $shortDocument
 if ((Query 19) -ne -1 -or (Query 25) -ne -1 -or (Query 31) -ne 0) { throw 'A fitting short document still exposes a scrollbar' }
 Capture 'short-document'

 StopReader
 $paragraphDocument = Join-Path $Output 'cached-paragraph.md'
 [IO.File]::WriteAllText($paragraphDocument, ('lianzy wrapped paragraph row ' * 140), [Text.UTF8Encoding]::new($false))
 StartReader $paragraphDocument
 # One physical line below the 4096-byte segment limit has source offset zero
 # throughout: its wrapped-row ordinal must move independently of that offset.
 $paragraphSize = Query 10
 if ($paragraphSize -ge 4096 -or (Cursor) -ne 0) { throw 'Invalid complete-cache paragraph fixture' }
 $paragraphByte = Drag 0.5
 $paragraphMiddleRow = Query 14
 if ($paragraphByte -ne 0 -or $paragraphMiddleRow -le 0) { throw 'Cached paragraph thumb did not move through wrapped rows' }
 Capture 'cached-paragraph-middle'
 # Theme-only changes and clicking an already selected mode must not jump a wrapped paragraph.
 foreach ($command in @(209,208,210,211,201,409)) {
  [void][ScrollbarSmoke]::SendMessageW($window, 0x111, [IntPtr]$command, [IntPtr]::Zero)
  Settle
  if ((Query 9) -ne $paragraphByte -or (Query 14) -ne $paragraphMiddleRow) {
   throw "Header command $command moved the reading position: wrapped row $paragraphMiddleRow -> $(Query 14)"
  }
 }

 $paragraphEndByte = Drag 1.0
 $paragraphEndRow = Query 14
 if ($paragraphEndByte -ne 0 -or $paragraphEndRow -le $paragraphMiddleRow) { throw 'Cached paragraph thumb did not reach the last full viewport' }
 $paragraphTopByte = Drag 0.0
 if ($paragraphTopByte -ne 0 -or (Cursor) -ne 0) { throw 'Cached paragraph thumb did not return to its first wrapped row' }

 $metrics = [ordered]@{
  result = 'PASS'; author = 'lianzy'; company = 'lianzy'; last_modified_by = 'lianzy'
  input = 'native window messages; hidden process; no desktop input injection'
  fixture_bytes = $size; vertical_middle_byte = $middle; vertical_bottom_byte = $bottom
  vertical_double_click_middle_byte = $doubleClickMiddle
  track_page_cursor = $pageCursor; wheel_cursor = $wheelCursor
  horizontal_max = $horizontalMax; horizontal_middle = $horizontalMiddle; horizontal_end = $horizontalEnd
  capture_cancel_axes = 2; rapid_drag_round_trips = $roundTrip; short_document_scrollbars_hidden = $true
  theme_and_selected_mode_preserve_wrapped_row = $true
  cached_paragraph_bytes = $paragraphSize; cached_paragraph_middle_row = $paragraphMiddleRow; cached_paragraph_end_row = $paragraphEndRow
  exe_bytes = (Get-Item -LiteralPath $Exe).Length
 }
 $metrics | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Output 'scrollbar.json') -Encoding utf8
 $metrics | ConvertTo-Json
} finally {
 try {
  StopReader
 } finally {
  if ($createdIni) { Remove-Item -LiteralPath $ini -ErrorAction SilentlyContinue }
  $env:FEATHERMD_TEST_SNAPSHOT = $oldSnapshot
  if ($oldDpiContext -ne [IntPtr]::Zero) { [void][ScrollbarSmoke]::SetThreadDpiAwarenessContext($oldDpiContext) }
 }
}
