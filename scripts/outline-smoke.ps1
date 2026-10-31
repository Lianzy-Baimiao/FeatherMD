param([ValidateSet('light','dark')][string]$Theme = 'light', [string]$Exe = "$PSScriptRoot\..\target\release\feathermd.exe", [string]$Output = "$PSScriptRoot\..\test-artifacts\outline", [switch]$AllowDesktopUi, [switch]$SecondaryMonitor)
# The outline as a tree (depths from section numbers, folds), its filter box, the
# contents list linking to headings, and Find's results panel.
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/require-ui-test-host.ps1" -AllowDesktopUi:$AllowDesktopUi -SecondaryMonitor:$SecondaryMonitor -Exe $Exe
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class OutlineSmoke {
 public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr data);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr hwnd, StringBuilder text, int size);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")] public static extern IntPtr SendText(IntPtr hwnd, uint msg, IntPtr wp, string text);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern int GetWindowTextLengthW(IntPtr hwnd);
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
$snapshot = Join-Path $Output 'outline.bmp'
$outlineFile = Join-Path $Output 'outline.txt'
$process = $null; $window = [IntPtr]::Zero; $createdIni = $false; $dpi = 96
$oldDpi = [OutlineSmoke]::SetThreadDpiAwarenessContext([IntPtr](-4))

# A converted specification: a contents list of plain items, then headings all at
# ####/##### whose hierarchy lives in their numbers. 3 modules x 2 groups x 30 forms,
# two tables per form, two sections per table: 1 + 9 + 180 entries to the forms,
# 360 tables more than the 500 an outline opens with, so forms start folded.
$toc = [Collections.Generic.List[string]]::new()
$body = [Collections.Generic.List[string]]::new()
# Author-written links first: one to a heading, one to nothing.
$toc.Add('见[不存在的标题](#nowhere)与[第一个模块](#1-模块1)。'); $toc.Add('')
$toc.Add('## 目 录'); $toc.Add('')
foreach ($m in 1..3) {
 $toc.Add("* $m 模块$m"); $body.Add("#### $m 模块$m"); $body.Add('')
 foreach ($g in 1..2) {
  $toc.Add("* $m.$g 分组$m$g"); $body.Add("#### $m.$g 分组$m$g"); $body.Add('')
  foreach ($f in 1..30) {
   $name = "表单 $m.$g.$f"
   $toc.Add("* $m.$g.$f $name"); $body.Add("#### $m.$g.$f $name"); $body.Add('')
   foreach ($t in 1..2) {
    $body.Add("#### $name-表$t"); $body.Add('')
    $body.Add('##### 概述'); $body.Add('')
    $body.Add('|  |  |'); $body.Add('| --- | --- |'); $body.Add('| 表名称 | 实体名称 |'); $body.Add("| T_$m${g}_$f$t | 表$t |"); $body.Add('')
    $body.Add('##### 字段'); $body.Add('')
    $body.Add('| 标识 | 类型 |'); $body.Add('| --- | --- |'); $body.Add("| UNIQ$m${g}x$f | int |"); $body.Add('')
   }
  }
 }
}
$document = Join-Path $Output 'spec.md'
$all = [Collections.Generic.List[string]]::new($toc); $all.Add(''); $all.AddRange($body)
[IO.File]::WriteAllText($document, ($all -join "`n"), $utf8)
$forms = 180

function Query([int]$id, [int]$index = 0) { [OutlineSmoke]::SendMessageW($window, 0x802a, [IntPtr]$id, [IntPtr]$index).ToInt64() }
function Send([IntPtr]$h, [uint32]$msg, [int]$wp, [IntPtr]$lp) { [void][OutlineSmoke]::SendMessageW($h, $msg, [IntPtr]$wp, $lp) }
function Post([IntPtr]$h, [uint32]$msg, [int]$wp) { [void][OutlineSmoke]::PostMessageW($h, $msg, [IntPtr]$wp, [IntPtr]::Zero) }
function Scale([int]$value) { [int][Math]::Floor($value * $dpi / 96) }
# Types $text into box $h one character at a time, as an IME commits text.
function Enter-Text([IntPtr]$h, [string]$text) { foreach ($c in $text.ToCharArray()) { [void][OutlineSmoke]::PostMessageW($h, 0x0286, [IntPtr][int]$c, [IntPtr]::Zero) } }
function Idle {
 for ($i = 0; $i -lt 500; $i++) {
  if ($process.HasExited) { throw 'Reader exited unexpectedly' }
  if ((Query 11) -eq 0 -and (Query 2) -gt 0) { return }
  Start-Sleep -Milliseconds 10
 }
 throw 'Layout timed out'
}
function Settle { for ($i = 0; $i -lt 6; $i++) { Start-Sleep -Milliseconds 40; [void](Query 0) }; Idle }
function Capture([string]$name = '') {
 if ([OutlineSmoke]::IsWindowVisible($window)) { throw 'Reader became visible' }
 if ((Query 4) -ne 1) { throw 'Internal capture failed' }
 if ($name) {
  $bytes = [IO.File]::ReadAllBytes($snapshot)
  $stream = [IO.MemoryStream]::new($bytes)
  $bitmap = [Drawing.Bitmap]::new($stream)
  try { $bitmap.Save((Join-Path $Output ($name + $suffix + '.png')), [Drawing.Imaging.ImageFormat]::Png) } finally { $bitmap.Dispose(); $stream.Dispose() }
 }
}
# A click at client point $point (y << 16 | x, as the centre queries return it).
function ClickAt([long]$point) {
 if ($point -lt 0) { throw 'Nothing painted there to click' }
 Capture
 $p = [IntPtr]$point
 Send $window 0x201 1 $p; Send $window 0x202 0 $p
 Idle
}
# What the reader would put in the status line, and the URL of the link at $point.
function Status { [void](Query 130); return [IO.File]::ReadAllText((Join-Path $Output 'status.txt'), $utf8) }
function LinkAt([long]$point) { [void](Query 131 $point); return [IO.File]::ReadAllText((Join-Path $Output 'link.txt'), $utf8) }
function RowPoint([int]$row) {
 $y = (Query 110) + ($row - (Query 36)) * (Query 111) + [Math]::Floor((Query 111) / 2)
 return ([long]$y -shl 16) -bor (Scale 120)
}
# The sidebar row that lists entry $entry, or -1.
function RowOf([int]$entry) {
 for ($r = 0; $r -lt (Query 112); $r++) { if ((Query 113 $r) -eq $entry) { return $r } }
 return -1
}
function StartReader([string]$path) {
 [IO.File]::WriteAllText($ini, "theme=$Theme`nfont_size=16`nwrap=true`noutline=true`nlanguage=zh-CN`n", $utf8)
 $script:createdIni = $true
 $env:FEATHERMD_TEST_SNAPSHOT = $snapshot
 $script:process = Start-Process -FilePath $Exe -ArgumentList ('"' + $path + '"') -WindowStyle Hidden -PassThru
 $script:window = [IntPtr]::Zero
 for ($i = 0; $i -lt 200 -and $window -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 25; $script:window = [OutlineSmoke]::Find($process.Id) }
 if ($window -eq [IntPtr]::Zero) { throw 'Reader window not created' }
 Idle
 $script:dpi = [int](Query 3)
 for ($i = 0; $i -lt 1000 -and (Query 33) -ne 0; $i++) { Start-Sleep -Milliseconds 10 }
 if ((Query 33) -ne 0) { throw 'Outline did not finish' }
 Settle
}
function StopReader {
 if ($process -and -not $process.HasExited) {
  Post $window 0x10 0
  if (-not $process.WaitForExit(5000)) { $process.Kill(); throw 'Reader failed to exit cleanly' }
 }
}
$suffix = if ($Theme -eq 'dark') { '-dark' } else { '' }
try {
 StartReader $document
 # Entries: the contents title, then modules, groups, forms, tables, sections.
 $total = 1 + 9 + $forms * (1 + 2 * 3)
 if ((Query 34) -ne $total) { throw "Outline has $(Query 34) entries, expected $total" }
 Remove-Item -LiteralPath $outlineFile -ErrorAction SilentlyContinue
 if ((Query 102) -ne $total) { throw 'Outline seam unavailable' }
 $texts = @([IO.File]::ReadAllText($outlineFile, $utf8) -split "`n" | ForEach-Object { ($_ -split "`t", 4)[3] })
 $title = [Array]::IndexOf($texts, '目 录'); $module = [Array]::IndexOf($texts, '2 模块2')
 $group = [Array]::IndexOf($texts, '2.1 分组21'); $deep = [Array]::IndexOf($texts, '2.1.15 表单 2.1.15')
 $form = [Array]::IndexOf($texts, '1.1.3 表单 1.1.3'); $moduleOne = [Array]::IndexOf($texts, '1 模块1')
 foreach ($e in @(@($title, 1), @($module, 1), @($group, 2), @($deep, 3), @($form, 3))) {
  $row = RowOf $e[0]
  if ($row -lt 0) { throw "Entry $($e[0]) ($($texts[$e[0]])) is not listed" }
  if ((Query 114 $row) -ne $e[1]) { throw "$($texts[$e[0]]) has depth $(Query 114 $row), expected $($e[1])" }
 }
 # Opened folded to the forms.
 $listed = Query 112
 if ($listed -ne 1 + 9 + $forms) { throw "Outline lists $listed rows, expected the $(1 + 9 + $forms) entries down to the forms" }
 $formRow = RowOf $form
 if ((Query 115 $formRow) -ne 1) { throw 'A form with tables did not start folded' }
 Capture 'outline-tree'
 # Unfold one form: its two tables appear, themselves folded.
 $mark = Query 128 $formRow
 if ($mark -lt 0) { throw "Form row $formRow shows no fold mark (top row $(Query 36))" }
 ClickAt $mark
 if ((Query 112) -ne $listed + 2 -or (Query 115 $formRow) -ne 0) { throw "Unfolding listed $(Query 112) rows" }
 if ((Query 115 ($formRow + 1)) -ne 1 -or (Query 114 ($formRow + 1)) -ne 4) { throw 'An unfolded form does not list its folded tables' }
 Capture 'outline-unfolded'
 ClickAt (Query 128 $formRow)
 if ((Query 112) -ne $listed) { throw 'Folding the form again did not hide its tables' }

 # The filter box lists matches with the parents they need, folds aside.
 $filter = [IntPtr](Query 116)
 if ($filter -eq [IntPtr]::Zero) { throw 'No filter box' }
 [void][OutlineSmoke]::SendText($filter, 0x0c, [IntPtr]::Zero, '表单 2.1.15')
 Settle
 # The form and its two tables match; the module and group are context.
 if ((Query 117) -ne 3 -or (Query 112) -ne 5) { throw "Filter matched $(Query 117) of $(Query 112) rows, expected 3 of 5" }
 if ((Query 113 0) -ne $module -or (Query 113 2) -ne $deep) { throw 'Filter rows are not the parents then the match' }
 Capture 'outline-filter'
 Post $filter 0x100 0x0d
 Settle
 if ((Query 9) -ne (Query 37 $deep)) { throw "Enter in the filter went to $(Query 9), not the form at $(Query 37 $deep)" }
 [void][OutlineSmoke]::SendText($filter, 0x0c, [IntPtr]::Zero, 'nothing like this')
 Settle
 if ((Query 112) -ne 0) { throw 'A filter without matches still lists rows' }
 Capture 'outline-filter-none'
 Post $filter 0x100 0x1b
 Settle
 if ([OutlineSmoke]::GetWindowTextLengthW($filter) -ne 0 -or (Query 112) -ne $listed) { throw "Esc did not clear the filter ($(Query 112) rows)" }
 # Characters committed one by one filter as they arrive.
 Enter-Text $filter '分组21'
 Settle
 if ((Query 117) -ne 1 -or (Query 112) -ne 2) { throw "Typed 分组21 matched $(Query 117) of $(Query 112) rows" }
 Post $filter 0x100 0x1b
 Settle

 # The contents list links to the headings it names, indented by their depth.
 Send $window 0x0100 0x24 ([IntPtr]::Zero)
 Settle
 for ($i = 0; $i -lt 200 -and (Query 126) -lt 1; $i++) { Start-Sleep -Milliseconds 20; Capture }
 if ((Query 126) -lt 5) { throw "The page shows $(Query 126) links inside the document" }
 Capture 'contents-links'
 # Links in paint order: the two written ones, then the contents list.
 $urls = @(0..3 | ForEach-Object { LinkAt (Query 127 $_) })
 $expected = @('#nowhere', '#1-模块1', '#1-模块1', '#11-分组11')
 if (($urls -join '|') -cne ($expected -join '|')) { throw "Links point to $($urls -join ', ')" }
 ClickAt (Query 127 0)
 if ((Query 9) -ne 0 -or (Status) -cne '文档中没有与此链接对应的标题：#nowhere') { throw "A link to no heading moved to $(Query 9): $(Status)" }
 ClickAt (Query 127 1)
 if ((Query 9) -ne (Query 37 $moduleOne)) { throw "The written link went to $(Query 9), not 1 模块1 at $(Query 37 $moduleOne)" }
 Send $window 0x0100 0x24 ([IntPtr]::Zero)
 Settle
 Capture
 ClickAt (Query 127 3)
 $groupEntry = [Array]::IndexOf($texts, '1.1 分组11')
 if ((Query 9) -ne (Query 37 $groupEntry)) { throw "The second contents link went to $(Query 9), not 1.1 at $(Query 37 $groupEntry)" }
 if ((Query 46) -ne $groupEntry) { throw "The outline did not follow the link: active $(Query 46)" }
 # Numbered headings at one level read in sizes by depth: 1.1 is the second level.
 if ((Query 145 (Query 13)) -ne 2) { throw "1.1 分组11 is drawn as heading level $(Query 145 (Query 13))" }
 # An empty header row gives way: the overview table is headed by 表名称.
 $first = -1
 for ($r = Query 13; $r -lt (Query 2) -and $first -lt 0; $r++) { if ((Query 146 $r) -ge 0) { $first = $r } }
 if ($first -lt 0 -or (Query 146 $first) -ne 1) { throw 'No table header near 1.1 分组11' }
 $headerText = [IO.File]::ReadAllText((Join-Path $Output 'row.txt'), $utf8)
 if ($headerText -cne "表名称`t实体名称") { throw "The first table row is '$headerText', not the overview header" }
 Capture 'contents-link-followed'

 # Find lists every match at the right; Enter steps through them.
 Send $window 0x0100 0x24 ([IntPtr]::Zero)
 Settle
 Send $window 0x111 102 ([IntPtr]::Zero)
 $edit = [IntPtr](Query 7)
 if ((Query 123) -le 0) { throw 'Find opened without its results panel' }
 Capture 'find-empty'
 # Typed in lower case: letters match either case unless Aa is on.
 Enter-Text $edit 'uniq2'
 for ($i = 0; $i -lt 300 -and (Query 118) -ne 120; $i++) { Start-Sleep -Milliseconds 20 }
 # UNIQ2 is module 2's field: 2 groups x 30 forms x 2 tables.
 if ((Query 118) -ne 120 -or (Query 119) -ne 0) { throw "Find listed $(Query 118) matches (more: $(Query 119)), expected 120" }
 if ((Query 120) -ne -1) { throw 'Typing moved to a match' }
 Settle
 $toggle = Query 143
 ClickAt $toggle
 for ($i = 0; $i -lt 300 -and (Query 118) -ne 0; $i++) { Start-Sleep -Milliseconds 20 }
 if ((Query 144) -ne 1 -or (Query 118) -ne 0) { throw "Match case found $(Query 118) of uniq2" }
 Capture 'find-match-case'
 ClickAt (Query 143)
 for ($i = 0; $i -lt 300 -and (Query 118) -ne 120; $i++) { Start-Sleep -Milliseconds 20 }
 if ((Query 144) -ne 0 -or (Query 118) -ne 120) { throw "Either case again found $(Query 118)" }
 Settle
 $rowsBeforeJumps = Query 112
 Capture 'find-results'
 Post $edit 0x100 0x0d
 Settle
 if ((Query 120) -ne 0 -or (Query 9) -gt (Query 121 0) -or (Query 5) -ne (Query 121 0)) { throw "Enter went to match $(Query 120) at $(Query 5), expected the first at $(Query 121 0)" }
 Post $edit 0x100 0x0d
 Settle
 if ((Query 120) -ne 1) { throw "A second Enter is at match $(Query 120)" }
 Capture
 ClickAt (Query 125 5)
 if ((Query 120) -ne 5 -or (Query 5) -ne (Query 121 5)) { throw "Clicking result 5 went to match $(Query 120)" }
 Capture 'find-current'
 if ((Status) -cnotlike '第 6 / 120 处 · 第 * 行') { throw "Status after the result click: $(Status)" }
 # The match is marked in the text, and jumping did not unfold the outline.
 if ((Query 129) -ne 1) { throw 'The current match was not marked in the text' }
 if ((Query 112) -ne $rowsBeforeJumps) { throw "Find jumps unfolded the outline: $rowsBeforeJumps -> $(Query 112) rows" }
 $line = Query 122 5
 if ($line -lt 1) { throw 'Result without a line number' }
 Post $edit 0x100 0x1b
 Settle
 if ((Query 123) -ne 0) { throw 'Esc left the results panel open' }
 StopReader

 $metrics = [ordered]@{
  result = 'PASS'; theme = $Theme; author = 'lianzy'; dpi = $dpi
  outline_entries = $total; rows_opened = $listed; depths_checked = 5; fold_toggle = $true
  filter_matches = 3; filter_enter_seek = $true; filter_escape = $true
  contents_links_followed = $true; written_links = 2; missing_link_reported = $true; typed_filter = $true; typed_find = $true; match_case_toggle = $true; empty_header_promoted = $true; heading_level_by_depth = $true; find_matches = 120; find_enter_steps = $true; find_click = $true; result_line = $line; match_marked = $true; jumps_keep_folds = $true
  screenshots = @("outline-tree$suffix.png", "outline-unfolded$suffix.png", "outline-filter$suffix.png", "outline-filter-none$suffix.png", "contents-links$suffix.png", "contents-link-followed$suffix.png", "find-empty$suffix.png", "find-results$suffix.png", "find-match-case$suffix.png", "find-current$suffix.png")
  limitations = 'Hidden window with synthetic messages; typed text arrives as committed WM_IME_CHAR characters, without an IME composition window.'
  exe_bytes = (Get-Item -LiteralPath $Exe).Length
 }
 $metrics | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Output 'outline.json') -Encoding utf8
 $metrics | ConvertTo-Json
} finally {
 try { StopReader } finally {
  if ($createdIni) { Remove-Item -LiteralPath $ini -ErrorAction SilentlyContinue }
  $env:FEATHERMD_TEST_SNAPSHOT = $oldSnapshot
  if ($oldDpi -ne [IntPtr]::Zero) { [void][OutlineSmoke]::SetThreadDpiAwarenessContext($oldDpi) }
 }
}

. "$PSScriptRoot/verify-ui-test-host.ps1" -Output $Output -SecondaryMonitor:$SecondaryMonitor
