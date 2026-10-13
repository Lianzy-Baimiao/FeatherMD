param([string]$Exe = "$PSScriptRoot\..\target\release\feathermd.exe", [string]$Output = "$PSScriptRoot\..\test-artifacts\i18n")
# Interface languages: every view in zh-CN, zh-TW, en and ja must show its own text
# and nothing may be cut off; Save switches the language live without a restart.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class I18nSmoke {
 public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr data);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr hwnd, StringBuilder text, int size);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern int GetWindowLongW(IntPtr hwnd, int index);
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr hwnd, int id);
 [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process, uint flag);
 [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd, int command);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
 [DllImport("kernel32.dll")] public static extern ushort GetUserDefaultUILanguage();
 [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")] static extern IntPtr ReadText(IntPtr hwnd, uint msg, IntPtr wp, StringBuilder text);
 public static string Text(IntPtr hwnd) { var s = new StringBuilder(4096); ReadText(hwnd, 0x000d, (IntPtr)s.Capacity, s); return s.ToString(); }
 public static IntPtr Point(int x, int y) { return new IntPtr(((long)(ushort)y << 16) | (ushort)x); }
 public static IntPtr Find(uint pid, string cls, string title) {
  IntPtr found = IntPtr.Zero;
  EnumWindows((h,d) => {
   uint p; GetWindowThreadProcessId(h, out p);
   var c = new StringBuilder(128); GetClassNameW(h, c, c.Capacity);
   if (p == pid && c.ToString() == cls && (String.IsNullOrEmpty(title) || Text(h) == title)) { found = h; return false; }
   return true;
  }, IntPtr.Zero);
  return found;
 }
}
"@
$Exe = (Resolve-Path -LiteralPath $Exe).Path
New-Item -ItemType Directory -Path $Output -Force | Out-Null
$Output = (Resolve-Path -LiteralPath $Output).Path
$ini = Join-Path (Split-Path $Exe) 'FeatherMD.ini'
if (Test-Path -LiteralPath $ini) { throw 'Refusing to overwrite existing portable settings' }
$utf8 = [Text.UTF8Encoding]::new($false)
$snapshot = Join-Path $Output 'i18n.bmp'
$labelsFile = Join-Path $Output 'labels.txt'
$menusFile = Join-Path $Output 'menus.txt'
$oldSnapshot = $env:FEATHERMD_TEST_SNAPSHOT
$process = $null; $window = [IntPtr]::Zero; $dialog = [IntPtr]::Zero; $createdIni = $false; $dpi = 96
$oldDpi = [I18nSmoke]::SetThreadDpiAwarenessContext([IntPtr](-4))
$fixture = Join-Path $Output 'languages.md'
[IO.File]::WriteAllLines($fixture, [string[]]@(
 '# 多语言 / Languages / 言語', '', '每种语言的界面文字都必须完整显示。', '',
 '```text', 'Copy me', '```', '',
 '> 引用 / Quote / 引用', '',
 '| 语言 | Language |', '| --- | --- |', '| 简体中文 | Simplified Chinese |', '| 日本語 | Japanese |'
), $utf8)
# Expected text per language, in the table order of src/i18n/strings.rs.
$languages = @(
 @{ Code = 'zh-CN'; Caption = 'FeatherMD 羽读'; Settings = '设置'; About = '关于 FeatherMD'; Menus = @("设置…`tCtrl+,", '界面语言 / Language…', '检查更新…', '帮助(&H)') },
 @{ Code = 'zh-TW'; Caption = 'FeatherMD 羽讀'; Settings = '設定'; About = '關於 FeatherMD'; Menus = @("設定…`tCtrl+,", '介面語言 / Language…', '檢查更新…', '說明(&H)') },
 @{ Code = 'en'; Caption = 'FeatherMD'; Settings = 'Settings'; About = 'About FeatherMD'; Menus = @("Settings…`tCtrl+,", 'Language…', 'Check for updates…', '&Help') },
 @{ Code = 'ja'; Caption = 'FeatherMD'; Settings = '設定'; About = 'FeatherMD について'; Menus = @("設定…`tCtrl+,", '表示言語 / Language…', '更新を確認…', 'ヘルプ(&H)') }
)
$problems = [Collections.Generic.List[string]]::new()
$checked = 0
function Query([int]$id, [int]$index = 0) { [I18nSmoke]::SendMessageW($window, 0x802a, [IntPtr]$id, [IntPtr]$index).ToInt64() }
function Send([IntPtr]$h, [uint32]$msg, [int]$wp, [IntPtr]$lp) { [void][I18nSmoke]::SendMessageW($h, $msg, [IntPtr]$wp, $lp) }
function Post([IntPtr]$h, [uint32]$msg, [int]$wp) { if (-not [I18nSmoke]::PostMessageW($h, $msg, [IntPtr]$wp, [IntPtr]::Zero)) { throw 'Posting a native message failed' } }
function Scale([int]$value) { [int][Math]::Floor($value * $dpi / 96) }
function Problem([string]$text) { $problems.Add($text); Write-Host "PROBLEM $text" }
function WaitWindow([string]$class, [string]$title) {
 for ($i = 0; $i -lt 200; $i++) {
  if ($process.HasExited) { throw 'Reader exited unexpectedly' }
  $h = [I18nSmoke]::Find($process.Id, $class, $title)
  if ($h -ne [IntPtr]::Zero) { return $h }
  Start-Sleep -Milliseconds 25
 }
 throw "Window not found: $class / $title"
}
function Idle {
 for ($i = 0; $i -lt 500; $i++) {
  if ($process.HasExited) { throw 'Reader exited unexpectedly' }
  if ((Query 11) -eq 0 -and ((Query 10) -eq 0 -or (Query 2) -gt 0)) { return }
  Start-Sleep -Milliseconds 10
 }
 throw 'Reader did not settle'
}
function StartReader([string]$language, [string]$document = '') {
 [IO.File]::WriteAllText($ini, "theme=light`nfont_size=18`nlanguage=$language`n", $utf8)
 $script:createdIni = $true
 $env:FEATHERMD_TEST_SNAPSHOT = $snapshot
 if ($document) {
  $script:process = Start-Process -FilePath $Exe -ArgumentList ('"' + $document + '"') -WindowStyle Hidden -PassThru
 } else {
  $script:process = Start-Process -FilePath $Exe -WindowStyle Hidden -PassThru
 }
 $script:window = WaitWindow 'FeatherMD.Native.Reader' $null
 for ($j = 0; $j -lt 200 -and (Query 0) -le 0; $j++) { Start-Sleep -Milliseconds 10 }
 Idle
 # The outline is built in the background; captures should show the finished one.
 if ($document) {
  for ($j = 0; $j -lt 500 -and (Query 33) -ne 0; $j++) { Start-Sleep -Milliseconds 10 }
  if ((Query 33) -ne 0) { throw 'Outline did not finish' }
 }
 $script:dpi = [int](Query 3)
}
function StopReader {
 if ($process -and -not $process.HasExited) {
  Post $window 0x10 0
  if (-not $process.WaitForExit(5000)) { $process.Kill(); throw 'Reader failed to exit cleanly' }
 }
}
# Paints $h into the snapshot, keeps it as a PNG and reports text that does not fit.
function Check([IntPtr]$h, [string]$name) {
 if ([I18nSmoke]::IsWindowVisible($h)) { throw "$name became visible" }
 if ([I18nSmoke]::SendMessageW($h, 0x802a, [IntPtr]4, [IntPtr]::Zero).ToInt64() -ne 1) { throw "Capture failed: $name" }
 $bitmap = [Drawing.Bitmap]::new($snapshot)
 try { $bitmap.Save((Join-Path $Output ($name + '.png')), [Drawing.Imaging.ImageFormat]::Png) } finally { $bitmap.Dispose() }
 $query = if ($h -eq $window) { 78 } else { 5 }
 $count = [I18nSmoke]::SendMessageW($h, 0x802a, [IntPtr]$query, [IntPtr]::Zero).ToInt64()
 if ($count -lt 0) { throw "Overflow diagnostics unavailable: $name" }
 if ($count -gt 0) {
  foreach ($line in [IO.File]::ReadAllLines($labelsFile, $utf8)) { Problem "$name cuts off: $line" }
 }
 $script:checked++
}
function Flush { [void][I18nSmoke]::SendMessageW($dialog, 0, [IntPtr]::Zero, [IntPtr]::Zero); Start-Sleep -Milliseconds 30 }
function OpenDialog([int]$command, [string]$title, [int]$ready) {
 Post $window 0x111 $command
 $script:dialog = WaitWindow '#32770' $title
 for ($j = 0; $j -lt 100 -and [I18nSmoke]::GetDlgItem($dialog, $ready) -eq [IntPtr]::Zero; $j++) { Start-Sleep -Milliseconds 20 }
 # The modal loop shows a dialog when it first goes idle, which can come after an
 # early hide; hide until it stays hidden.
 for ($quiet = 0; $quiet -lt 5) {
  if ([I18nSmoke]::IsWindowVisible($dialog)) { [void][I18nSmoke]::ShowWindow($dialog, 0); $quiet = 0 } else { $quiet++ }
  Flush
 }
}
function CloseDialog {
 Post $dialog 0x10 0
 for ($i = 0; $i -lt 200 -and [I18nSmoke]::IsWindow($dialog); $i++) { Start-Sleep -Milliseconds 20 }
 if ([I18nSmoke]::IsWindow($dialog)) { throw 'Dialog failed to close' }
 $script:dialog = [IntPtr]::Zero
}
function Item([int]$id) { $h = [I18nSmoke]::GetDlgItem($dialog, $id); if ($h -eq [IntPtr]::Zero) { throw "Control $id missing" }; return $h }
function HeaderLeft { return ((Query 0) - (Scale 132) - (Scale 236) - 1) }
# Hovers every header and caption control until its tooltip shows.
function HeaderTips([string]$prefix) {
 $left = HeaderLeft; $width = [int](Query 0)
 $points = @(18, 50, 102, 138, 175, 212 | ForEach-Object { $left + (Scale $_) })
 $points += @(0, 1, 2 | ForEach-Object { $width - (Scale 132) - 1 + (Scale ($_ * 44 + 22)) })
 $n = 0
 foreach ($x in $points) {
  if ((Query 4) -ne 1) { throw 'Paint before hover failed' }
  Send $window 0x200 0 ([I18nSmoke]::Point($x, (Scale 17)))
  Start-Sleep -Milliseconds 600
  if ((Query 43) -ne 1) { Problem "$prefix tooltip $n did not appear (hover $(Query 44))" }
  Check $window ("$prefix-tip$n")
  Send $window 0x2a3 0 ([IntPtr]::Zero)
  $n++
 }
}
# Every view of one language: welcome, header tooltips, appearance picker, menus,
# all Settings pages, About (both states), a document with its copy button, Find
# and Go to. A fresh process each time, so nothing is left from another language.
function Views([int]$index) {
 $expect = $languages[$index]; $code = $expect.Code
 StartReader $code
 if ((Query 77) -ne $index) { Problem "$code started as language $(Query 77)" }
 $caption = [I18nSmoke]::Text($window)
 if ($caption -cne $expect.Caption) { Problem "$code window caption '$caption', expected '$($expect.Caption)'" }
 Check $window "$code-welcome"
 HeaderTips $code
 # Sent, not posted: Windows delivers sent messages first, so a posted command
 # could run after the next query. Only commands that open a modal loop are posted.
 Send $window 0x111 412 ([IntPtr]::Zero)
 if ((Query 42) -lt 0) { Problem "$code appearance picker did not open" }
 Check $window "$code-picker"
 Send $window 0x100 0x1b ([IntPtr]::Zero)
 # Menus are rebuilt per language; the More menu is a popup of the same tree.
 $count = Query 79
 if ($count -le 0) { throw "Menu diagnostics failed for $code" }
 $menus = [IO.File]::ReadAllLines($menusFile, $utf8)
 Copy-Item -LiteralPath $menusFile -Destination (Join-Path $Output "$code-menus.txt") -Force
 foreach ($label in $expect.Menus) {
  if ($menus -cnotcontains $label) { Problem "$code menus lack '$label'" }
 }
 # Settings, page by page.
 OpenDialog 415 $expect.Settings 1103
 for ($page = 0; $page -lt 5; $page++) {
  Send (Item (1110 + $page)) 0xf5 0 ([IntPtr]::Zero); Flush
  Check $dialog "$code-settings-$page"
 }
 CloseDialog
 # About, collapsed and expanded.
 OpenDialog 401 $expect.About 1003
 Check $dialog "$code-about"
 Send (Item 1003) 0xf5 0 ([IntPtr]::Zero)
 for ($i = 0; $i -lt 100 -and ([I18nSmoke]::GetWindowLongW((Item 2010), -16) -band 0x10000000) -eq 0; $i++) { Start-Sleep -Milliseconds 20 }
 Flush
 Check $dialog "$code-about-expanded"
 CloseDialog
 StopReader
 # A document: footer, outline, copy button (before and after a click), Find, Go to.
 StartReader $code $fixture
 Check $window "$code-reading"
 $row = Query 69 0
 $y = Query 61 $row
 Send $window 0x200 0 ([I18nSmoke]::Point(((Query 59 $row) + (Scale 4)), $y))
 Check $window "$code-copy"
 if ((Query 71) -lt 0) {
  Problem "$code shows no copy button over the code block"
 } else {
  $x = [int](((Query 71) + (Query 73)) / 2); $by = [int](((Query 72) + (Query 74)) / 2)
  Send $window 0x200 0 ([I18nSmoke]::Point($x, $by))
  Send $window 0x201 1 ([I18nSmoke]::Point($x, $by))
  Send $window 0x202 0 ([I18nSmoke]::Point($x, $by))
  Check $window "$code-copied"
  if ((Query 76) -ne 1) { Problem "$code copy button did not show its check mark" }
 }
 Send $window 0x2a3 0 ([IntPtr]::Zero)
 Send $window 0x111 102 ([IntPtr]::Zero)
 Check $window "$code-find"
 Send $window 0x100 0x1b ([IntPtr]::Zero)
 Send $window 0x111 103 ([IntPtr]::Zero)
 Check $window "$code-goto"
 Send $window 0x100 0x1b ([IntPtr]::Zero)
 StopReader
 # The demo document in this language, as its README shows it.
 $name = if ($code -eq 'zh-CN') { 'demo.md' } else { "demo.$code.md" }
 StartReader $code (Resolve-Path "$PSScriptRoot\..\docs\$name").Path
 Check $window "$code-demo"
 StopReader
}
function Handles([int]$kind) { $min = [int]::MaxValue; for ($j = 0; $j -lt 12; $j++) { Start-Sleep -Milliseconds 5; $min = [Math]::Min($min, [I18nSmoke]::GetGuiResources($process.Handle, $kind)) }; return $min }
# Saves language $target on the Settings language page of the running reader.
function SwitchTo([int]$target, [string]$name) {
 $from = [int](Query 77)
 OpenDialog 418 $languages[$from].Settings 1103
 $list = Item 1170
 # The list starts with "System"; the languages follow in table order.
 Send $list 0x14e ($target + 1) ([IntPtr]::Zero)
 Send $dialog 0x111 (1170 -bor (1 -shl 16)) $list
 Post $dialog 0x111 1
 for ($i = 0; $i -lt 200 -and [I18nSmoke]::IsWindow($dialog); $i++) { Start-Sleep -Milliseconds 20 }
 $script:dialog = [IntPtr]::Zero
 Idle
 $code = $languages[$target].Code
 if ((Query 77) -ne $target) { Problem "Live switch to $code left language $(Query 77)" }
 $caption = [I18nSmoke]::Text($window)
 if ($caption -cne $languages[$target].Caption) { Problem "Live switch to ${code}: caption '$caption'" }
 if ((Query 79) -le 0) { throw 'Menu diagnostics failed' }
 $menus = [IO.File]::ReadAllLines($menusFile, $utf8)
 foreach ($label in $languages[$target].Menus) {
  if ($menus -cnotcontains $label) { Problem "Live switch to ${code}: menus lack '$label'" }
 }
 if ([IO.File]::ReadAllText($ini, $utf8) -notmatch "(?m)^language=$([regex]::Escape($code))\r?$") { Problem "Live switch to $code was not saved" }
 Check $window $name
}
try {
 for ($index = 0; $index -lt $languages.Count; $index++) { Views $index }

 # "System" follows the Windows display language; untranslated ones get English.
 $id = [int][I18nSmoke]::GetUserDefaultUILanguage()
 $primary = $id -band 0x3ff; $sub = $id -shr 10
 $systemIndex = 2
 if ($primary -eq 0x04) {
  $systemIndex = 0
  if (@(0x01, 0x03, 0x05, 0x1f) -contains $sub) { $systemIndex = 1 }
 } elseif ($primary -eq 0x11) { $systemIndex = 3 }
 StartReader 'system'
 if ((Query 77) -ne $systemIndex) { Problem ('System language 0x{0:x4} resolved to {1}, expected {2}' -f $id, (Query 77), $systemIndex) }
 OpenDialog 418 $languages[$systemIndex].Settings 1103
 Check $dialog 'system-settings-language'
 CloseDialog
 StopReader

 # Saving another language switches menus, captions and fonts without a restart.
 # The first round may allocate lasting objects; the second must not add more.
 StartReader 'zh-CN'
 $switches = 0
 foreach ($round in 0..1) {
  foreach ($target in @(1, 2, 3, 0)) {
   SwitchTo $target "switch-$round-$($languages[$target].Code)"
   $switches++
  }
  if ($round -eq 0) { $gdiBefore = Handles 0; $userBefore = Handles 1 }
 }
 $gdiAfter = Handles 0; $userAfter = Handles 1
 if ($gdiAfter -gt $gdiBefore + 2) { Problem "Language switches leaked GDI handles: $gdiBefore -> $gdiAfter" }
 if ($userAfter -gt $userBefore + 2) { Problem "Language switches leaked USER handles: $userBefore -> $userAfter" }
 StopReader

 $result = 'PASS'
 if ($problems.Count -gt 0) { $result = 'FAIL' }
 $metrics = [ordered]@{
  result = $result; author = 'lianzy'; dpi = $dpi
  languages = @($languages | ForEach-Object { $_.Code })
  system_ui_language = ('0x{0:x4}' -f $id); system_resolves_to = $languages[$systemIndex].Code
  views_checked = $checked; live_switches = $switches
  gdi_before = $gdiBefore; gdi_after = $gdiAfter; user_before = $userBefore; user_after = $userAfter
  problems = @($problems)
  limitations = 'Hidden windows and native messages. Text is measured where FeatherMD draws it and in STATIC controls; ellipsis text, native message boxes and the system file dialogs are not measured. The system case checks only this machine''s display language.'
  exe_bytes = (Get-Item -LiteralPath $Exe).Length
 }
 $metrics | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Output 'i18n.json') -Encoding utf8
 $metrics | ConvertTo-Json
 if ($problems.Count -gt 0) { throw "$($problems.Count) interface language problem(s); see i18n.json" }
} finally {
 if ($dialog -ne [IntPtr]::Zero -and [I18nSmoke]::IsWindow($dialog)) { [void][I18nSmoke]::PostMessageW($dialog, 0x10, [IntPtr]::Zero, [IntPtr]::Zero); Start-Sleep -Milliseconds 100 }
 if ($process -and -not $process.HasExited) { [void][I18nSmoke]::PostMessageW($window, 0x10, [IntPtr]::Zero, [IntPtr]::Zero); if (-not $process.WaitForExit(5000)) { $process.Kill() } }
 $env:FEATHERMD_TEST_SNAPSHOT = $oldSnapshot
 if ($createdIni) { Remove-Item -LiteralPath $ini -ErrorAction SilentlyContinue }
 if ($oldDpi -ne [IntPtr]::Zero) { [void][I18nSmoke]::SetThreadDpiAwarenessContext($oldDpi) }
}
