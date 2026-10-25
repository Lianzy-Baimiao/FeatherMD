param([ValidateSet('light','dark')][string]$Theme = 'light', [string]$Exe = "$PSScriptRoot\..\target\release\feathermd.exe", [string]$Output = "$PSScriptRoot\..\test-artifacts\formats", [switch]$AllowDesktopUi, [switch]$SecondaryMonitor)
# Plain text and logs: how each format is read and what its outline lists.
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/require-ui-test-host.ps1" -AllowDesktopUi:$AllowDesktopUi -SecondaryMonitor:$SecondaryMonitor -Exe $Exe
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class FormatsSmoke {
 public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr data);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr hwnd, StringBuilder text, int size);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process, uint flag);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
 [DllImport("kernel32.dll")] public static extern uint GetACP();
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
$snapshot = Join-Path $Output 'formats.bmp'
$copyFile = Join-Path $Output 'copy.txt'
$outlineFile = Join-Path $Output 'outline.txt'
$process = $null; $window = [IntPtr]::Zero; $createdIni = $false; $dpi = 96
$oldDpi = [FormatsSmoke]::SetThreadDpiAwarenessContext([IntPtr](-4))
$indent = [string][char]0x3000 + [char]0x3000
# Forty paragraphs per chapter, so a clicked chapter can scroll to the top.
function Body([string]$tag) { 1..40 | ForEach-Object { $indent + "这一段写的是$tag 的第 $_ 件事，平平常常。" } }
$novelLines = [string[]](@(
 '书名'
 ''
 '第一卷 风起'
 ''
 '第一章 少年'
 ($indent + '他走出了家门，天还没有亮。')
 '一、这只是正文里的清单'
) + (Body 'a') + @(
 ''
 '第二章：重逢'
 ($indent + '多年以后，他们又见面了。')
) + (Body 'b') + @(
 ''
 '第二卷'
 '第三章'
 ($indent + '故事继续。')
) + (Body 'c') + @(
 'Chapter 4: The Storm'
 'The wind rose.'
) + (Body 'd'))
$novel = Join-Path $Output 'novel.txt'
[IO.File]::WriteAllText($novel, ($novelLines -join "`r`n"), $utf8)
# Busy minutes keep the index small: 08:00 and 08:11 each hold many lines.
$logLines = [string[]](@(
 '2026-10-23 23:59:58,101 [main] WARN  com.example.Disk - low disk'
 '2026-10-24 08:00:01,002 [main] INFO  com.example.Api - start'
 '2026-10-24 08:00:30,513 [worker-1] ERROR com.example.Job - failed'
 'java.lang.IllegalStateException: boom'
 "`tat com.example.Job.run(Job.java:42)"
) + (1..120 | ForEach-Object { '2026-10-24 08:00:40,{0:D3} [worker-3] INFO  com.example.Api - request {0} done' -f $_ }) + @(
 '2026-10-24 08:10:00,000 [worker-2] ERROR com.example.Job - again'
) + (1..120 | ForEach-Object { '2026-10-24 08:11:00,{0:D3} [worker-2] INFO  com.example.Job - step {0} done' -f $_ }))
$serverLog = Join-Path $Output 'server.log'
[IO.File]::WriteAllText($serverLog, ($logLines -join "`n"), $utf8)
# The same lines in a .txt: recognised as a log by its content.
$logText = Join-Path $Output 'export.txt'
[IO.File]::WriteAllText($logText, ($logLines -join "`n"), $utf8)
$buildLog = Join-Path $Output 'build.log'
[IO.File]::WriteAllText($buildLog, ([string[]]@(
 'Compiling foo v0.1.0'
 'warning: unused variable `x`'
 'error[E0308]: mismatched types'
 'src/main.rs:3:5: error: expected one'
 'Traceback (most recent call last):'
 '  File "a.py", line 1'
) -join "`n"), $utf8)
# A GBK (GB18030) novel, as many older Chinese .txt files are.
try { $gbk = [Text.Encoding]::GetEncoding(54936) } catch {
 [Text.Encoding]::RegisterProvider([Text.CodePagesEncodingProvider]::Instance)
 $gbk = [Text.Encoding]::GetEncoding(54936)
}
$gbkNovel = Join-Path $Output 'novel-gbk.txt'
[IO.File]::WriteAllBytes($gbkNovel, $gbk.GetBytes(($novelLines -join "`r`n")))
function Query([int]$id, [int]$index = 0) { [FormatsSmoke]::SendMessageW($window, 0x802a, [IntPtr]$id, [IntPtr]$index).ToInt64() }
function Send([uint32]$msg, [int]$wp, [IntPtr]$lp) { [void][FormatsSmoke]::SendMessageW($window, $msg, [IntPtr]$wp, $lp) }
function Scale([int]$value) { [int][Math]::Floor($value * $dpi / 96) }
function Idle {
 for ($i = 0; $i -lt 500; $i++) {
  if ($process.HasExited) { throw 'Reader exited unexpectedly' }
  if ((Query 11) -eq 0 -and (Query 2) -gt 0) { return }
  Start-Sleep -Milliseconds 10
 }
 throw 'Layout timed out'
}
# Waits for the outline scan, then reads its entries (level, errors, warnings, text).
function Outline {
 for ($i = 0; $i -lt 1000 -and (Query 33) -ne 0; $i++) { Start-Sleep -Milliseconds 10 }
 if ((Query 33) -ne 0) { throw 'Outline did not finish' }
 Capture
 Remove-Item -LiteralPath $outlineFile -ErrorAction SilentlyContinue
 $count = Query 102
 if ($count -lt 0) { throw 'Outline seam unavailable' }
 if ($count -eq 0) { return @() }
 return @([IO.File]::ReadAllText($outlineFile, $utf8) -split "`n")
}
function StartReader([string]$path) {
 # Fixed interface language: CI runners follow an English system locale.
 [IO.File]::WriteAllText($ini, "theme=$Theme`nfont_size=18`nwrap=true`noutline=true`nlanguage=zh-CN`n", $utf8)
 $script:createdIni = $true
 $env:FEATHERMD_TEST_SNAPSHOT = $snapshot
 $script:process = Start-Process -FilePath $Exe -ArgumentList ('"' + $path + '"') -WindowStyle Hidden -PassThru
 $script:window = [IntPtr]::Zero
 for ($i = 0; $i -lt 200 -and $window -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 25; $script:window = [FormatsSmoke]::Find($process.Id) }
 if ($window -eq [IntPtr]::Zero) { throw 'Reader window not created' }
 Idle
 $script:dpi = [int](Query 3)
}
function StopReader {
 if ($process -and -not $process.HasExited) {
  [void][FormatsSmoke]::PostMessageW($window, 0x10, [IntPtr]::Zero, [IntPtr]::Zero)
  if (-not $process.WaitForExit(5000)) { $process.Kill(); throw 'Reader failed to exit cleanly' }
 }
}
function Capture([string]$name = '') {
 if ([FormatsSmoke]::IsWindowVisible($window)) { throw 'Reader became visible' }
 if ((Query 4) -ne 1) { throw 'Internal capture failed' }
 if ($name) {
  $bytes = [IO.File]::ReadAllBytes($snapshot)
  $stream = [IO.MemoryStream]::new($bytes)
  $bitmap = [Drawing.Bitmap]::new($stream)
  try { $bitmap.Save((Join-Path $Output ($name + '.png')), [Drawing.Imaging.ImageFormat]::Png) } finally { $bitmap.Dispose(); $stream.Dispose() }
 }
}
function Copied {
 Remove-Item -LiteralPath $copyFile -ErrorAction SilentlyContinue
 if ((Query 62) -lt 0) { throw 'Copy seam unavailable' }
 return [IO.File]::ReadAllText($copyFile, $utf8)
}
# Clicks outline entry $index (when it is among the visible rows) and checks the seek.
function ClickEntry([int]$index) {
 $offset = Query 37 $index
 Capture
 $x = Scale 40; $y = Scale (90 + 30 * $index + 15)
 Send 0x201 1 ([FormatsSmoke]::Point($x, $y)); Send 0x202 0 ([FormatsSmoke]::Point($x, $y))
 Idle
 if ((Query 9) -ne $offset) { throw "Outline entry $index did not seek to byte $offset; reading at $(Query 9)" }
 return $offset
}
function Expect([string]$what, $actual, $expected) {
 if (($actual -join "`n") -cne ($expected -join "`n")) { throw "$what`n  expected: $($expected -join ' | ')`n  actual:   $($actual -join ' | ')" }
}
$suffix = if ($Theme -eq 'dark') { '-dark' } else { '' }
try {
 # Plain text: chapters (volumes level 1, chapters level 2), numbering inside a
 # chapter left out; lines kept as written, indents included.
 StartReader $novel
 if ((Query 100) -ne 1) { throw "novel.txt read as format $(Query 100), not text" }
 $entries = Outline
 if ((Query 101) -ne 1) { throw "novel.txt outline kind $(Query 101), not chapters" }
 Expect 'Novel chapters' $entries @("1`t0`t0`t第一卷 风起", "2`t0`t0`t第一章 少年", "2`t0`t0`t第二章：重逢", "1`t0`t0`t第二卷", "2`t0`t0`t第三章", "2`t0`t0`tChapter 4: The Storm")
 $copied = Copied
 if (-not $copied.Contains($indent + '他走出了家门') -or -not $copied.Contains('第一章 少年')) { throw "Novel copy lost its lines: $copied" }
 Capture ('formats-novel' + $suffix)
 $chapter = ClickEntry 2
 $at = [Array]::IndexOf($novelLines, '第二章：重逢')
 if ($chapter -ne $utf8.GetByteCount((($novelLines[0..($at - 1)]) -join "`r`n") + "`r`n")) { throw "Chapter offset $chapter is not the start of 第二章" }
 StopReader

 # A log: a time index of days and time ranges with error and warning counts.
 StartReader $serverLog
 if ((Query 100) -ne 2) { throw "server.log read as format $(Query 100), not a log" }
 $entries = Outline
 if ((Query 101) -ne 2) { throw "server.log outline kind $(Query 101), not a time index" }
 Expect 'Log time index' $entries @("1`t0`t1`t2026-10-23", "2`t0`t1`t23:59", "1`t2`t0`t2026-10-24", "2`t1`t0`t08:00", "2`t1`t0`t08:10", "2`t0`t0`t08:11")
 Capture ('formats-log' + $suffix)
 $range = ClickEntry 3
 if ($range -ne ($utf8.GetByteCount($logLines[0]) + 1)) { throw "Time range 08:00 seeks to $range, not its first line" }
 # Source mode shows the exact text, tabs included.
 Send 0x111 205 ([IntPtr]::Zero); Idle
 if ((Query 38) -ne 1) { throw 'Source mode did not start' }
 if (-not (Copied).Contains("`tat com.example.Job.run")) { throw 'Source copy of the log lost its tab' }
 Capture ('formats-log-source' + $suffix)
 Send 0x111 205 ([IntPtr]::Zero); Idle
 StopReader

 # A .txt of log lines is read as a log; a log without timestamps lists errors.
 StartReader $logText
 if ((Query 100) -ne 2) { throw "export.txt of log lines read as format $(Query 100)" }
 if (@(Outline).Count -ne 6 -or (Query 101) -ne 2) { throw 'export.txt has no time index' }
 StopReader
 StartReader $buildLog
 $entries = Outline
 if ((Query 101) -ne 3) { throw "build.log outline kind $(Query 101), not error lines" }
 Expect 'Build log errors' $entries @("1`t1`t0`terror[E0308]: mismatched types", "1`t1`t0`tsrc/main.rs:3:5: error: expected one", "1`t1`t0`tTraceback (most recent call last):")
 Capture ('formats-build' + $suffix)
 StopReader

 # A GBK novel: decoded through the system's Chinese code page (GB18030) where that
 # is the legacy page; elsewhere it must still open.
 $acp = [FormatsSmoke]::GetACP()
 StartReader $gbkNovel
 if ((Query 100) -ne 1) { throw 'novel-gbk.txt not read as text' }
 $gbkChapters = 0
 if ($acp -eq 936) {
  if ((Query 103) -ne 54936) { throw "GBK novel encoding $(Query 103), not GB18030" }
  $entries = Outline
  Expect 'GBK novel chapters' $entries @("1`t0`t0`t第一卷 风起", "2`t0`t0`t第一章 少年", "2`t0`t0`t第二章：重逢", "1`t0`t0`t第二卷", "2`t0`t0`t第三章", "2`t0`t0`tChapter 4: The Storm")
  if (-not (Copied).Contains($indent + '他走出了家门')) { throw 'GBK novel did not decode' }
  $gbkChapters = $entries.Count
 }
 Capture ('formats-gbk' + $suffix)
 StopReader

 $metrics = [ordered]@{
  result = 'PASS'; theme = $Theme; author = 'lianzy'; dpi = $dpi; system_code_page = $acp
  novel_chapters = 6; novel_numbering_dropped = $true; novel_indent_kept = $true; chapter_seek = $chapter
  log_time_entries = 6; log_errors_per_day = '0,2'; log_range_seek = $range; log_source_keeps_tab = $true
  txt_sniffed_as_log = $true; build_log_error_lines = 3; gbk_chapters = $gbkChapters
  screenshots = @("formats-novel$suffix.png", "formats-log$suffix.png", "formats-log-source$suffix.png", "formats-build$suffix.png", "formats-gbk$suffix.png")
  limitations = 'Hidden window with synthetic messages; GBK decoding is checked only where the system code page is 936.'
  exe_bytes = (Get-Item -LiteralPath $Exe).Length
 }
 $metrics | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Output 'formats.json') -Encoding utf8
 $metrics | ConvertTo-Json
} finally {
 try { StopReader } finally {
  if ($createdIni) { Remove-Item -LiteralPath $ini -ErrorAction SilentlyContinue }
  $env:FEATHERMD_TEST_SNAPSHOT = $oldSnapshot
  if ($oldDpi -ne [IntPtr]::Zero) { [void][FormatsSmoke]::SetThreadDpiAwarenessContext($oldDpi) }
 }
}

. "$PSScriptRoot/verify-ui-test-host.ps1" -Output $Output -SecondaryMonitor:$SecondaryMonitor
