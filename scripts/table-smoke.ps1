param([ValidateSet('light','dark')][string]$Theme = 'light', [string]$Exe = "$PSScriptRoot\..\target\release\feathermd.exe", [string]$Output = "$PSScriptRoot\..\test-artifacts\table")
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class TableSmoke {
 public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr data);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr hwnd, StringBuilder text, int size);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int width, int height, uint flags);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process, uint flag);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
 public static IntPtr Point(int x, int y) { return new IntPtr(((long)(ushort)y << 16) | (ushort)x); }
 public static IntPtr Find(uint pid) {
  IntPtr found = IntPtr.Zero;
  EnumWindows((h,d) => { uint p; GetWindowThreadProcessId(h, out p); var c = new StringBuilder(128); GetClassNameW(h, c, c.Capacity); if (p == pid && c.ToString() == "FeatherMD.Native.Reader") { found = h; return false; } return true; }, IntPtr.Zero);
  return found;
 }
}
"@
New-Item -ItemType Directory -Path $Output -Force | Out-Null
$Output = (Resolve-Path -LiteralPath $Output).Path
$Exe = (Resolve-Path -LiteralPath $Exe).Path
$ini = Join-Path (Split-Path $Exe) 'FeatherMD.ini'
if (Test-Path -LiteralPath $ini) { throw 'Refusing to overwrite existing portable settings' }
$utf8 = [Text.UTF8Encoding]::new($false)
$oldSnapshot = $env:FEATHERMD_TEST_SNAPSHOT
$snapshot = Join-Path $Output 'table.bmp'
$copyFile = Join-Path $Output 'copy.txt'
$process = $null; $window = [IntPtr]::Zero; $createdIni = $false; $dpi = 96
$oldDpi = [TableSmoke]::SetThreadDpiAwarenessContext([IntPtr](-4))

$tables = Join-Path $Output 'tables.md'
[IO.File]::WriteAllLines($tables, [string[]]@(
 '# 表格'
 ''
 '表格按窗口宽度排版：放得下时保持内容宽度，放不下时先收窄最宽的列，单元格内自动换行。'
 ''
 '| 快捷键 | 功能 | 说明 |'
 '| :--- | :---: | :--- |'
 '| `Ctrl+F` | 查找 | 在整份文档中后台查找，区分大小写；按 F3 跳到下一处，Esc 取消。 |'
 '| `Ctrl+R` | 源码 | 在预览与源码之间切换，源码模式可以按字符选择并复制原文。 |'
 '| `Ctrl+B` | 目录 | 显示或隐藏左侧目录；窄窗口里目录会自动变窄，正文与表格随之重新排版。 |'
 '| `F5` | 重新载入 | 外部编辑保存后重新读取文件，阅读位置保持不变。 |'
 ''
 '## 宽表格'
 ''
 '列数太多、连最窄的列宽都放不下时，表格单独横向滚动，正文保持不动。'
 ''
 '| 参数一 | 参数二 | 参数三 | 参数四 | 参数五 | 参数六 | 参数七 | 参数八 |'
 '| --- | --- | --- | --- | --- | --- | --- | ---: |'
 '| averylongidentifier | anotherlongidentifier | thirdlongidentifier | fourthlongidentifier | fifthlongidentifier | sixthlongidentifier | seventhidentifier | 1024 |'
 '| a | b | c | d | e | f | g | 8 |'
 ''
 '表格之后的正文。'
), $utf8)
$source = Join-Path $Output 'source.md'
$sourceLines = [string[]]@('alpha beta gamma delta', 'second line here', 'one two three', '中文与 emoji 😀 混排')
[IO.File]::WriteAllLines($source, $sourceLines, $utf8)
$long = Join-Path $Output 'long-source.md'
[IO.File]::WriteAllLines($long, [string[]](0..299 | ForEach-Object { 'row {0:D3} of the source selection test' -f $_ }), $utf8)

function Query([int]$id, [int]$index = 0) { [TableSmoke]::SendMessageW($window, 0x802a, [IntPtr]$id, [IntPtr]$index).ToInt64() }
function Send([uint32]$msg, [int]$wp, [IntPtr]$lp) { [void][TableSmoke]::SendMessageW($window, $msg, [IntPtr]$wp, $lp) }
function Scale([int]$value) { [int][Math]::Floor($value * $dpi / 96) }
function Idle {
 for ($i = 0; $i -lt 500; $i++) {
  if ($process.HasExited) { throw 'Reader exited unexpectedly' }
  if ((Query 11) -eq 0 -and (Query 2) -gt 0) { return }
  Start-Sleep -Milliseconds 10
 }
 throw 'Layout timed out'
}
function StartReader([string]$path, [string]$settings) {
 [IO.File]::WriteAllText($ini, $settings, $utf8)
 $script:createdIni = $true
 $env:FEATHERMD_TEST_SNAPSHOT = $snapshot
 $script:process = Start-Process -FilePath $Exe -ArgumentList ('"' + $path + '"') -WindowStyle Hidden -PassThru
 $script:window = [IntPtr]::Zero
 for ($i = 0; $i -lt 200 -and $window -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 25; $script:window = [TableSmoke]::Find($process.Id) }
 if ($window -eq [IntPtr]::Zero) { throw 'Reader window not created' }
 Idle
 $script:dpi = [int](Query 3)
}
function StopReader {
 if ($process -and -not $process.HasExited) {
  [void][TableSmoke]::PostMessageW($window, 0x10, [IntPtr]::Zero, [IntPtr]::Zero)
  if (-not $process.WaitForExit(5000)) { $process.Kill(); throw 'Reader failed to exit cleanly' }
 }
}
# Paints (refreshing hit targets) and optionally keeps the frame as a PNG.
function Capture([string]$name = '') {
 if ([TableSmoke]::IsWindowVisible($window)) { throw 'Reader became visible' }
 if ((Query 4) -ne 1) { throw 'Internal capture failed' }
 if ($name) {
  $bitmap = [Drawing.Bitmap]::new($snapshot)
  try { $bitmap.Save((Join-Path $Output ($name + '.png')), [Drawing.Imaging.ImageFormat]::Png) } finally { $bitmap.Dispose() }
 }
}
function Copied {
 $bytes = Query 62
 if ($bytes -lt 0) { throw 'Copy seam unavailable' }
 $text = [IO.File]::ReadAllText($copyFile, $utf8)
 if ($utf8.GetByteCount($text) -ne $bytes) { throw "Copied length mismatch: $bytes vs $($utf8.GetByteCount($text))" }
 return $text
}
function Resize([int]$width, [int]$height) {
 if (-not [TableSmoke]::SetWindowPos($window, [IntPtr]::Zero, 0, 0, $width, $height, 0x16)) { throw 'Native resize failed' }
 Idle
}
function Row([int]$index) {
 $r = @{ Left = (Query 59 $index); Right = (Query 60 $index); Y = (Query 61 $index) }
 # A scrolled table may start left of the window; only Y says whether it is painted.
 if ($r.Y -lt 0) { throw "Layout row $index is not on screen" }
 return $r
}
function PointerDown([int]$x, [int]$y, [int]$keys = 1) { Send 0x201 $keys ([TableSmoke]::Point($x, $y)) }
function PointerMove([int]$x, [int]$y) { Send 0x200 1 ([TableSmoke]::Point($x, $y)) }
function PointerUp([int]$x, [int]$y) { Send 0x202 0 ([TableSmoke]::Point($x, $y)) }
function StableGdi { $min = [int]::MaxValue; for ($j = 0; $j -lt 12; $j++) { Start-Sleep -Milliseconds 5; $min = [Math]::Min($min, [TableSmoke]::GetGuiResources($process.Handle, 0)) }; return $min }
$suffix = if ($Theme -eq 'dark') { '-dark' } else { '' }

try {
 # Preview: two tables, one that fits the reading column and one that cannot.
 StartReader $tables "theme=$Theme`nfont_size=18`nwrap=true`n"
 Capture
 if ((Query 65) -ne 2) { throw "Expected 2 tables, found $(Query 65)" }
 if ((Query 58 0) -ne 3 -or (Query 58 1) -ne 8) { throw "Column counts: $(Query 58 0), $(Query 58 1)" }
 if ((Query 66 0) -ne 5 -or (Query 66 1) -ne 3) { throw "Row counts (header included): $(Query 66 0), $(Query 66 1)" }
 $body = Query 55
 $fitRight = Query 56 0
 if ($fitRight -gt $body) { throw "Shortcut table overflows the reading column: $fitRight > $body" }
 if ((Query 56 1) -le $body -or (Query 54) -ne (Query 56 1) -or (Query 31) -le 0) { throw "Wide table should scroll: right=$(Query 56 1) body=$body max=$(Query 31)" }
 $wideLines = Query 57 0
 $wideRight = Query 56 1
 Capture ('table-wide' + $suffix)
 # Horizontal scrolling moves only the table that does not fit; text and the
 # fitting table stay in place. Small screens (CI) scroll both tables into view.
 $wideRow = Query 64 1
 $fitRow = (Query 64 0) + (Query 57 0) - 1
 $textRow = $wideRow - 1
 for ($i = 0; $i -lt 40 -and (Query 61 $wideRow) -lt 0; $i++) { Send 0x100 0x28 ([IntPtr]::Zero); Idle; Capture }
 $textLeft = (Row $textRow).Left; $fitCell = (Row $fitRow).Left; $wideCell = (Row $wideRow).Left
 Send 0x114 7 ([IntPtr]::Zero)
 Capture ('table-scrolled' + $suffix)
 $moved = Query 18
 if ($moved -le 0 -or $moved -ne (Query 31)) { throw 'SB_RIGHT did not reach the end of the wide table' }
 if ((Row $textRow).Left -ne $textLeft) { throw 'Wrapped text moved with the table scroll' }
 if ((Row $fitRow).Left -ne $fitCell) { throw 'A table that fits moved with the wide table' }
 if ((Row $wideRow).Left -ne ($wideCell - $moved)) { throw "Wide table did not scroll by $moved px: $wideCell -> $((Row $wideRow).Left)" }
 Send 0x114 6 ([IntPtr]::Zero)
 Send 0x100 0x24 ([IntPtr]::Zero); Idle
 # Narrow window: the shortcut table still fits and wraps inside its cells.
 Resize (Scale 640) (Scale 480)
 Capture ('table-narrow' + $suffix)
 $narrowBody = Query 55
 if ((Query 56 0) -gt $narrowBody) { throw "Narrow shortcut table overflows: $(Query 56 0) > $narrowBody" }
 $narrowLines = Query 57 0
 if ($narrowLines -le (Query 66 0) -or $narrowLines -le $wideLines) { throw "Cells did not wrap in a narrow window: $wideLines -> $narrowLines lines for $(Query 66 0) rows" }
 # Without wrapping, tables keep their natural width and scroll with the text.
 Send 0x111 204 ([IntPtr]::Zero)
 Idle; Capture
 if ((Query 57 0) -ne (Query 66 0)) { throw 'Unwrapped table cells still wrap' }
 if ((Query 56 0) -le $narrowBody) { throw 'Unwrapped shortcut table did not keep its natural width' }
 $heading = (Row 0).Left
 Send 0x114 7 ([IntPtr]::Zero); Capture
 if ((Row 0).Left -ne ($heading - (Query 18))) { throw 'Unwrapped text did not scroll with the page' }
 Send 0x114 6 ([IntPtr]::Zero)
 Send 0x111 204 ([IntPtr]::Zero)
 Idle; Capture
 # Copy: table rows become tab-separated cells, each wrapped row once.
 Send 0x111 108 ([IntPtr]::Zero)
 $copied = Copied
 foreach ($expected in @("快捷键`t功能`t说明", "Ctrl+F`t查找`t在整份文档中", "a`tb`tc`td`te`tf`tg`t8")) {
  if (-not $copied.Contains($expected)) { throw "Copied table text lacks '$expected'" }
 }
 if (([regex]::Matches($copied, '窄窗口里目录会自动变窄')).Count -ne 1) { throw 'A wrapped table row was copied more than once' }
 if (-not $copied.Contains('表格之后的正文。')) { throw 'Copied text lost the paragraph after the table' }
 Send 0x100 0x1b ([IntPtr]::Zero)
 Resize (Scale 1080) (Scale 820)
 $tableGdi = StableGdi
 StopReader

 # Source mode: character selection over exact source text.
 StartReader $source "theme=$Theme`nfont_size=18`nwrap=true`nraw=true`n"
 Capture
 if ((Query 38) -ne 1) { throw 'Source mode did not start' }
 $r0 = Row 0; $r2 = Row 2
 $cw = ($r0.Right - $r0.Left) / $sourceLines[0].Length
 PointerDown ([int]($r0.Left + 6 * $cw)) $r0.Y
 PointerMove ([int]($r2.Left + 2 * $cw)) (Row 1).Y
 PointerMove ([int]($r2.Left + 4 * $cw)) $r2.Y
 PointerUp ([int]($r2.Left + 4 * $cw)) $r2.Y
 $expected = "beta gamma delta`r`nsecond line here`r`none "
 if ((Copied) -cne $expected) { throw "Drag selection copied '$(Copied)'" }
 if ((Query 63) -ne 1) { throw 'Drag left no selection' }
 Capture ('source-selection' + $suffix)
 # Double-click selects the word under the pointer.
 $x = [int]($r0.Left + 7.5 * $cw)
 PointerDown $x $r0.Y; PointerUp $x $r0.Y
 Send 0x203 1 ([TableSmoke]::Point($x, $r0.Y)); PointerUp $x $r0.Y
 if ((Copied) -cne 'beta') { throw "Double-click selected '$(Copied)'" }
 # Shift+click extends from the caret, across CJK text and an emoji.
 $r3 = Row 3
 PointerDown $r3.Left $r3.Y; PointerUp $r3.Left $r3.Y
 PointerDown $r3.Right $r3.Y 5; PointerUp $r3.Right $r3.Y
 if ((Copied) -cne $sourceLines[3]) { throw "Shift+click copied '$(Copied)'" }
 # Select all, then Esc; without a selection Copy takes the visible source.
 Send 0x111 108 ([IntPtr]::Zero)
 if ((Copied) -cne ($sourceLines -join "`r`n")) { throw 'Select all did not copy the page source' }
 Send 0x100 0x1b ([IntPtr]::Zero)
 if ((Query 63) -ne 0) { throw 'Esc did not clear the selection' }
 if ((Copied) -cne ($sourceLines -join "`r`n")) { throw 'Copy without a selection did not take the visible source' }
 Capture
 # The first selection creates a few lasting GDI objects; repeats must not add more.
 $gdiBefore = StableGdi
 for ($round = 0; $round -lt 4; $round++) {
  PointerDown ([int]($r0.Left + 6 * $cw)) $r0.Y; PointerMove ([int]($r2.Left + 4 * $cw)) $r2.Y; PointerUp ([int]($r2.Left + 4 * $cw)) $r2.Y
  [void](Copied); Capture
  PointerDown $x $r0.Y; PointerUp $x $r0.Y; Send 0x203 1 ([TableSmoke]::Point($x, $r0.Y)); PointerUp $x $r0.Y
  PointerDown $r3.Left $r3.Y; PointerUp $r3.Left $r3.Y; PointerDown $r3.Right $r3.Y 5; PointerUp $r3.Right $r3.Y
  [void](Copied); Capture
  Send 0x111 108 ([IntPtr]::Zero); Send 0x100 0x1b ([IntPtr]::Zero); [void](Copied); Capture
 }
 $gdiAfter = StableGdi
 # Four repeats: a per-round leak exceeds the same small tolerance update-smoke uses.
 if ($gdiAfter -gt $gdiBefore + 2) { throw "Source selection leaked GDI handles: $gdiBefore -> $gdiAfter" }
 StopReader

 # Dragging below the text scrolls and keeps extending the selection.
 StartReader $long "theme=$Theme`nfont_size=18`nwrap=true`nraw=true`n"
 Capture
 $start = Row 0
 PointerDown ($start.Left + 1) $start.Y
 $below = (Query 1) - (Scale 20)
 PointerMove ($start.Left + 40) $below
 for ($i = 0; $i -lt 100 -and (Query 13) -lt 20; $i++) { Start-Sleep -Milliseconds 20 }
 $scrolled = Query 13
 PointerUp ($start.Left + 40) $below
 if ($scrolled -lt 20) { throw "Dragging below the text did not scroll: top=$scrolled" }
 $copiedLines = (Copied) -split "`r`n"
 if ($copiedLines[0] -cne 'row 000 of the source selection test' -or $copiedLines.Count -lt 20) { throw "Auto-scroll selection copied $($copiedLines.Count) lines starting '$($copiedLines[0])'" }
 StopReader

 $metrics = [ordered]@{
  result = 'PASS'; theme = $Theme; author = 'lianzy'; dpi = $dpi
  tables = 2; reading_width = $body; fit_table_right = $fitRight; wide_table_right = $wideRight
  fit_table_lines = $wideLines; narrow_reading_width = $narrowBody; narrow_table_lines = $narrowLines
  wide_table_scrolls_alone = $true; unwrapped_tables_natural_width = $true; table_copy_tab_separated = $true
  source_drag_exact = $true; source_double_click_word = $true; source_shift_click_cjk_emoji = $true
  source_select_all_and_esc = $true; source_drag_autoscroll_rows = $scrolled; source_autoscroll_copied_lines = $copiedLines.Count
  table_gdi = $tableGdi; gdi_before = $gdiBefore; gdi_after = $gdiAfter
  screenshots = @("table-wide$suffix.png", "table-scrolled$suffix.png", "table-narrow$suffix.png", "source-selection$suffix.png")
  limitations = 'Hidden window with synthetic mouse messages; real pointer input and the system clipboard are not exercised (Copy is read through the test seam).'
  exe_bytes = (Get-Item -LiteralPath $Exe).Length
 }
 $metrics | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Output 'table.json') -Encoding utf8
 $metrics | ConvertTo-Json
} finally {
 try { StopReader } finally {
  if ($createdIni) { Remove-Item -LiteralPath $ini -ErrorAction SilentlyContinue }
  $env:FEATHERMD_TEST_SNAPSHOT = $oldSnapshot
  if ($oldDpi -ne [IntPtr]::Zero) { [void][TableSmoke]::SetThreadDpiAwarenessContext($oldDpi) }
 }
}
