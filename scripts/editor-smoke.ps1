param([string]$Exe = "$PSScriptRoot\..\target\release\feathermd.exe", [string]$Probe = "$PSScriptRoot\..\target\release\examples\editor_probe.exe", [string]$Output = "$PSScriptRoot\..\test-artifacts\editor", [switch]$AllowNoPopup, [switch]$AllowDesktopUi, [switch]$SecondaryMonitor)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/require-ui-test-host.ps1" -AllowDesktopUi:$AllowDesktopUi -SecondaryMonitor:$SecondaryMonitor -Exe $Exe
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class EditorSmoke {
 public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
 public struct Point { public int X, Y; }
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr data);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr hwnd, StringBuilder text, int size);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hwnd, ref Point point);
 [DllImport("user32.dll")] public static extern int GetMenuItemCount(IntPtr menu);
 [DllImport("user32.dll")] public static extern uint GetMenuState(IntPtr menu, uint item, uint flags);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetMenuStringW(IntPtr menu, uint item, StringBuilder text, int max, uint flags);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
 [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")] static extern IntPtr ReadText(IntPtr hwnd, uint msg, IntPtr wp, StringBuilder text);
 public static string Text(IntPtr hwnd) { var s = new StringBuilder(4096); ReadText(hwnd, 0x000d, (IntPtr)s.Capacity, s); return s.ToString(); }
 public static string MenuText(IntPtr menu, uint position) { var s = new StringBuilder(512); GetMenuStringW(menu, position, s, s.Capacity, 0x400); return s.ToString(); }
 public static IntPtr Position(int x, int y) { return new IntPtr(((long)(ushort)y << 16) | (ushort)x); }
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
if (-not (Test-Path -LiteralPath $Probe)) { throw 'Build the probe first: cargo build --release --example editor_probe' }
$Probe = (Resolve-Path -LiteralPath $Probe).Path
New-Item -ItemType Directory -Path $Output -Force | Out-Null
$Output = (Resolve-Path -LiteralPath $Output).Path
$ini = Join-Path (Split-Path $Exe) 'FeatherMD.ini'
if (Test-Path -LiteralPath $ini) { throw 'Refusing to overwrite existing portable settings' }
# Spaces and CJK in the path prove the document reaches the editor as one argument.
$folder = Join-Path $Output '编辑 测试 by lianzy'
New-Item -ItemType Directory -Path $folder -Force | Out-Null
$document = Join-Path $folder '外部 编辑.md'
$lines = [string[]](0..5999 | ForEach-Object { '## Section {0:D4}' -f $_; ''; ('editable bounded paragraph {0} ' -f $_) * 6; '' })
[IO.File]::WriteAllLines($document, $lines, [Text.UTF8Encoding]::new($false))
$record = Join-Path $Output 'probe.txt'
$oldProbe = $env:FEATHERMD_EDITOR_PROBE
$oldSnapshot = $env:FEATHERMD_TEST_SNAPSHOT
$env:FEATHERMD_TEST_SNAPSHOT = Join-Path $Output 'editor.bmp'
$process = $null; $window = [IntPtr]::Zero; $createdIni = $false
$oldDpi = [EditorSmoke]::SetThreadDpiAwarenessContext([IntPtr](-4))
function Query([int]$id, [long]$lp = 0) { [EditorSmoke]::SendMessageW($window, 0x802a, [IntPtr]$id, [IntPtr]$lp).ToInt64() }
function Scale([int]$value) { [int][Math]::Floor($value * $dpi / 96) }
function WaitWindow([string]$class, [string]$title) {
 for ($i = 0; $i -lt 200; $i++) {
  if ($process.HasExited) { throw 'Reader exited unexpectedly' }
  $h = [EditorSmoke]::Find($process.Id, $class, $title)
  if ($h -ne [IntPtr]::Zero) { return $h }
  Start-Sleep -Milliseconds 25
 }
 throw "Window not found: $class / $title"
}
function Idle { for ($i = 0; $i -lt 500 -and ((Query 11) -ne 0 -or (Query 2) -eq 0); $i++) { Start-Sleep -Milliseconds 10 }; if ((Query 11) -ne 0) { throw 'Reader did not settle' } }
function StartReader([string]$settings) {
 [IO.File]::WriteAllText($ini, $settings + "`nlanguage=zh-CN`n", [Text.UTF8Encoding]::new($false)); $script:createdIni = $true
 $script:process = Start-Process -FilePath $Exe -ArgumentList @('"' + $document + '"') -WindowStyle Hidden -PassThru
 $script:window = WaitWindow 'FeatherMD.Native.Reader' $null
 Idle
 $script:dpi = [int](Query 3)
}
function StopReader { if ($process -and -not $process.HasExited) { [void][EditorSmoke]::PostMessageW($window, 0x10, [IntPtr]::Zero, [IntPtr]::Zero); if (-not $process.WaitForExit(5000)) { $process.Kill() } } }
function WaitProbe {
 for ($i = 0; $i -lt 200; $i++) { if (Test-Path -LiteralPath $record) { return [IO.File]::ReadAllLines($record, [Text.Encoding]::UTF8) }; Start-Sleep -Milliseconds 25 }
 throw 'The configured editor was not launched'
}
function AssertProbe([string[]]$received) {
 $args = @($received | Where-Object { $_.StartsWith('arg=') } | ForEach-Object { $_.Substring(4) })
 if ($args.Count -ne 1 -or $args[0] -ne $document) { throw "Editor received $($args.Count) argument(s): $($args -join ' | ')" }
 if (($received | Where-Object { $_.StartsWith('cwd=') }) -ne "cwd=$folder") { throw "Editor working directory: $($received -join ' | ')" }
}
function ContextMenu([int]$x, [int]$y) {
 $point = [EditorSmoke+Point]::new(); $point.X = $x; $point.Y = $y
 [void][EditorSmoke]::ClientToScreen($window, [ref]$point)
 [void][EditorSmoke]::PostMessageW($window, 0x7b, $window, [EditorSmoke]::Position($point.X, $point.Y))
 $menu = [IntPtr]::Zero
 # Only post keys once the menu's modal loop is showing it.
 for ($i = 0; $i -lt 200; $i++) {
  $menu = [EditorSmoke]::Find($process.Id, '#32768', $null)
  if ($menu -ne [IntPtr]::Zero -and [EditorSmoke]::IsWindowVisible($menu)) { return $menu }
  Start-Sleep -Milliseconds 10
 }
 # Some service desktops cannot show popups; the command path above is still verified.
 if ($AllowNoPopup) { [void][EditorSmoke]::PostMessageW($window, 0x1f, [IntPtr]::Zero, [IntPtr]::Zero); return [IntPtr]::Zero }
 throw 'Context menu did not appear'
}
try {
 $env:FEATHERMD_EDITOR_PROBE = $record
 Remove-Item -LiteralPath $record -ErrorAction SilentlyContinue
 StartReader "theme=light`nfont_size=18`neditor=$Probe`n"
 $width = Query 0; $height = Query 1

 # The text menu belongs to the reading area only.
 $text = @(((Query 35) + (Scale 120)), (Scale 200))
 if ((Query 50 ([long]([EditorSmoke]::Position($text[0], $text[1])))) -ne 1) { throw 'Reading area does not accept the text menu' }
 foreach ($outside in @(@((Scale 40), (Scale 200)), @(($width / 2), (Scale 17)), @(($width / 2), ($height - (Scale 10))))) {
  if ((Query 50 ([long]([EditorSmoke]::Position([int]$outside[0], [int]$outside[1])))) -ne 0) { throw "Text menu offered outside the text at $outside" }
 }

 # Command path: the document reaches the editor as one quoted argument.
 [void][EditorSmoke]::SendMessageW($window, 0x111, [IntPtr]107, [IntPtr]::Zero)
 AssertProbe (WaitProbe)
 Remove-Item -LiteralPath $record

 # Real native popup: label names the configured editor; keyboard selection launches it.
 $menuWindow = ContextMenu $text[0] $text[1]
 $popup = $menuWindow -ne [IntPtr]::Zero
 if ($popup) {
  $menu = [EditorSmoke]::SendMessageW($menuWindow, 0x1e1, [IntPtr]::Zero, [IntPtr]::Zero)
  if ($menu -eq [IntPtr]::Zero) { throw 'Context menu handle unavailable' }
  $count = [EditorSmoke]::GetMenuItemCount($menu)
  $labels = @(0..($count - 1) | ForEach-Object { [EditorSmoke]::MenuText($menu, $_) })
  if ($labels[0] -notmatch '^用 editor_probe 编辑\tCtrl\+E$') { throw "Edit item label: $($labels[0])" }
  foreach ($expected in @('选择编辑应用…', "复制可见内容`tCtrl+C", "全选当前分页`tCtrl+A", "查找…`tCtrl+F", "重新载入`tF5")) {
   if ($labels -notcontains $expected) { throw "Context menu lacks $expected; got $($labels -join ' | ')" }
  }
  if (([EditorSmoke]::GetMenuState($menu, 0, 0x400) -band 0x3) -ne 0) { throw 'Edit item is disabled with an open document' }
  [void][EditorSmoke]::PostMessageW($menuWindow, 0x100, [IntPtr]0x28, [IntPtr]::Zero)
  [void][EditorSmoke]::PostMessageW($menuWindow, 0x100, [IntPtr]0x0d, [IntPtr]::Zero)
  AssertProbe (WaitProbe)
  for ($i = 0; $i -lt 100 -and [EditorSmoke]::IsWindowVisible($menuWindow); $i++) { Start-Sleep -Milliseconds 20 }
  if ((Query 12) -ne 18) { throw 'Reader unresponsive after the context menu' }
  Remove-Item -LiteralPath $record

  # Escape dismisses the menu without launching anything.
  $menuWindow = ContextMenu $text[0] $text[1]
  [void][EditorSmoke]::PostMessageW($menuWindow, 0x100, [IntPtr]0x1b, [IntPtr]::Zero)
  for ($i = 0; $i -lt 100 -and [EditorSmoke]::IsWindowVisible($menuWindow); $i++) { Start-Sleep -Milliseconds 20 }
  if ([EditorSmoke]::IsWindowVisible($menuWindow)) { [void][EditorSmoke]::PostMessageW($window, 0x1f, [IntPtr]::Zero, [IntPtr]::Zero); throw 'Escape did not dismiss the context menu' }
  Start-Sleep -Milliseconds 150
  if (Test-Path -LiteralPath $record) { throw 'Dismissed menu launched the editor' }
 }

 # F5 keeps the reading position.
 [void][EditorSmoke]::SendMessageW($window, 0x111, [IntPtr]103, [IntPtr]::Zero)
 # Go-to opens with 50 already filled in.
 $edit = [IntPtr](Query 7)
 [void][EditorSmoke]::PostMessageW($edit, 0x100, [IntPtr]13, [IntPtr]::Zero)
 for ($i = 0; $i -lt 500 -and ((Query 11) -ne 0 -or [Math]::Abs((Query 9) / (Query 10) - 0.5) -gt 0.01); $i++) { Start-Sleep -Milliseconds 10 }
 $before = Query 9
 if ($before -lt 1000) { throw 'Go-to did not move into the document' }
 [void][EditorSmoke]::SendMessageW($window, 0x111, [IntPtr]104, [IntPtr]::Zero)
 for ($i = 0; $i -lt 500 -and ((Query 11) -ne 0 -or (Query 2) -eq 0 -or (Query 9) -eq 0); $i++) { Start-Sleep -Milliseconds 10 }
 $after = Query 9
 if ($after -ne $before) { throw "Reload moved the reader: $before -> $after" }
 StopReader

 # A missing editor explains itself and offers to choose another one.
 StartReader "theme=light`nfont_size=18`neditor=C:\FeatherMD missing\no-editor.exe`n"
 [void][EditorSmoke]::PostMessageW($window, 0x111, [IntPtr]107, [IntPtr]::Zero)
 $box = WaitWindow '#32770' '编辑'
 [void][EditorSmoke]::PostMessageW($box, 0x111, [IntPtr]6, [IntPtr]::Zero)
 $settings = WaitWindow '#32770' '设置'
 [void][EditorSmoke]::PostMessageW($settings, 0x100, [IntPtr]27, [IntPtr]::Zero)
 for ($i = 0; $i -lt 100 -and [EditorSmoke]::IsWindow($settings); $i++) { Start-Sleep -Milliseconds 20 }
 if (Test-Path -LiteralPath $record) { throw 'A missing editor still produced a launch' }
 if ((Query 12) -ne 18) { throw 'Reader unresponsive after the missing-editor prompt' }
 $metrics = [ordered]@{
  result = 'PASS'; author = 'lianzy'; quoted_unicode_space_path = $true; working_directory = $true
  native_context_menu_label_and_keyboard_launch = $popup; escape_dismisses_menu = $popup; menu_limited_to_text = $true
  reload_keeps_byte = $after; missing_editor_prompt_opens_settings = $true
  limitations = 'Uses a stand-in editor; real editors, Ctrl+E (needs a real keyboard state) and the Windows edit verb are not launched.'
 }
 $metrics | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Output 'editor.json') -Encoding utf8
 $metrics | ConvertTo-Json
} finally {
 StopReader
 $env:FEATHERMD_EDITOR_PROBE = $oldProbe
 $env:FEATHERMD_TEST_SNAPSHOT = $oldSnapshot
 if ($createdIni) { Remove-Item -LiteralPath $ini -ErrorAction SilentlyContinue }
 Remove-Item -LiteralPath $folder -Recurse -ErrorAction SilentlyContinue
 if ($oldDpi -ne [IntPtr]::Zero) { [void][EditorSmoke]::SetThreadDpiAwarenessContext($oldDpi) }
}

. "$PSScriptRoot/verify-ui-test-host.ps1" -Output $Output -SecondaryMonitor:$SecondaryMonitor
