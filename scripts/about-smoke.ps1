param([ValidateSet('light','dark','sepia','system')][string]$Theme = 'light', [string]$Exe = "$PSScriptRoot\..\target\release\feathermd.exe", [string]$Output = "$PSScriptRoot\..\test-artifacts\about")
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class AboutSmoke {
 public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
 public struct Rect { public int Left, Top, Right, Bottom; }
 public struct GuiInfo { public uint Size, Flags; public IntPtr Active, Focus, Capture, MenuOwner, MoveSize, Caret; public Rect CaretRect; }
 [DllImport("user32.dll")] static extern bool GetGUIThreadInfo(uint thread, ref GuiInfo info);
 public static IntPtr Focused(IntPtr hwnd) { uint pid; uint thread=GetWindowThreadProcessId(hwnd, out pid); var info=new GuiInfo { Size=(uint)Marshal.SizeOf(typeof(GuiInfo)) }; return GetGUIThreadInfo(thread, ref info) ? info.Focus : IntPtr.Zero; }
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr data);
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr hwnd, EnumProc proc, IntPtr data);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr hwnd, StringBuilder text, int size);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowTextW(IntPtr hwnd, StringBuilder text, int size);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern int GetWindowLongW(IntPtr hwnd, int index);
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr hwnd, int id);
 [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process, uint flag);
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hwnd, out Rect rect);
 [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd, int command);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
 public static string Text(IntPtr hwnd) { var s = new StringBuilder(4096); GetWindowTextW(hwnd, s, s.Capacity); return s.ToString(); }
 public static string Content(IntPtr hwnd) {
  var s = new StringBuilder(Text(hwnd));
  EnumChildWindows(hwnd, (h,d) => { s.AppendLine(Text(h)); return true; }, IntPtr.Zero);
  return s.ToString();
 }
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
$version = [Diagnostics.FileVersionInfo]::GetVersionInfo($Exe).FileVersion
New-Item -ItemType Directory -Path $Output -Force | Out-Null
$Output = (Resolve-Path -LiteralPath $Output).Path
$ini = Join-Path (Split-Path $Exe) 'FeatherMD.ini'
if (Test-Path -LiteralPath $ini) { throw 'Refusing to overwrite existing portable settings' }
$process = $null; $window = [IntPtr]::Zero; $dialog = [IntPtr]::Zero; $confirmation = [IntPtr]::Zero
$snapshot = Join-Path $Output 'about.bmp'
$oldSnapshot = $env:FEATHERMD_TEST_SNAPSHOT
$createdIni = $false
$oldDpi = [AboutSmoke]::SetThreadDpiAwarenessContext([IntPtr](-4))
function WaitWindow([string]$class, [string]$title) {
 for ($i = 0; $i -lt 100; $i++) {
  if ($process.HasExited) { throw 'Reader exited unexpectedly' }
  $h = [AboutSmoke]::Find($process.Id, $class, $title)
  if ($h -ne [IntPtr]::Zero) { return $h }
  Start-Sleep -Milliseconds 30
 }
 throw "Window not found: $class / $title"
}
function Post([IntPtr]$h, [uint32]$message, [int]$value) {
 if (-not [AboutSmoke]::PostMessageW($h, $message, [IntPtr]$value, [IntPtr]::Zero)) { throw 'Posting native message failed' }
}
function Capture([IntPtr]$h, [string]$name) {
 if ([AboutSmoke]::SendMessageW($h, 0x802a, [IntPtr]4, [IntPtr]::Zero).ToInt64() -ne 1) { throw 'About internal capture failed' }
 $bitmap = [Drawing.Bitmap]::new($snapshot)
 try {
  $color = $bitmap.GetPixel(4,4)
  $expected = switch ($Theme) { 'light' { '252,252,250' }; 'dark' { '25,29,34' }; 'sepia' { '247,239,221' }; default { $null } }
  if ($expected -and "$($color.R),$($color.G),$($color.B)" -ne $expected) { throw "Wrong rendered theme / blank capture: $color" }
  $bitmap.Save((Join-Path $Output $name), [Drawing.Imaging.ImageFormat]::Png)
 } finally { $bitmap.Dispose() }
}
function WaitClosed([IntPtr]$h) {
 for ($i = 0; $i -lt 100; $i++) { if (-not [AboutSmoke]::IsWindow($h)) { return }; Start-Sleep -Milliseconds 20 }
 throw 'Dialog failed to close'
}
try {
 $env:FEATHERMD_TEST_SNAPSHOT = $snapshot
 [IO.File]::WriteAllText($ini, "theme=$Theme`nlanguage=zh-CN`n", [Text.UTF8Encoding]::new($false)); $createdIni = $true
 $process = Start-Process -FilePath $Exe -WindowStyle Hidden -PassThru
 $window = WaitWindow 'FeatherMD.Native.Reader' $null
 Post $window 0x111 401
 $dialog = WaitWindow '#32770' '关于 FeatherMD'
 for ($i=0; $i -lt 100; $i++) {
  $default = [AboutSmoke]::SendMessageW($dialog, 0x400, [IntPtr]::Zero, [IntPtr]::Zero).ToInt64() -band 0xffff
  if ($default -eq 2) { break }; Start-Sleep -Milliseconds 20
 }
 if ($default -ne 2) { throw 'About must default to Return to reader' }
 # No desktop capture or pointer injection; hide only this test process's dialog.
 [void][AboutSmoke]::ShowWindow($dialog, 0)
 $content = [AboutSmoke]::Content($dialog)
 $content | Set-Content -LiteralPath (Join-Path $Output 'about-text.txt') -Encoding utf8
 foreach ($text in @('FeatherMD', "v$version", 'GitHub', '爱发电', 'MIT')) {
  if (-not $content.Contains($text)) { throw "About content missing: $text" }
 }
 if ([AboutSmoke]::IsWindowEnabled($window)) { throw 'About should be modal' }
 # App state must stay accessible during the modal loop (no outstanding RefCell borrow).
 $width = [AboutSmoke]::SendMessageW($window, 0x802a, [IntPtr]0, [IntPtr]::Zero).ToInt64()
 if ($width -lt 640) { throw 'About holds the reader state borrow during its modal loop' }
 Capture $dialog 'about.png'
 $compact = [AboutSmoke+Rect]::new(); [void][AboutSmoke]::GetWindowRect($dialog, [ref]$compact)
 Post ([AboutSmoke]::GetDlgItem($dialog, 1003)) 0xf5 0
 $expanded = [AboutSmoke+Rect]::new()
 for ($i=0; $i -lt 100; $i++) {
  [void][AboutSmoke]::GetWindowRect($dialog, [ref]$expanded)
  if (($expanded.Bottom-$expanded.Top) -gt ($compact.Bottom-$compact.Top)) { break }
  Start-Sleep -Milliseconds 20
 }
 if (($expanded.Bottom-$expanded.Top) -le ($compact.Bottom-$compact.Top)) { throw 'Shortcut panel did not expand' }
 if (-not ([AboutSmoke]::Content($dialog)).Contains('Ctrl+O')) { throw 'Shortcut information missing' }
 if (([AboutSmoke]::GetWindowLongW([AboutSmoke]::GetDlgItem($dialog,2010), -16) -band 0x10000000) -eq 0) { throw 'Expanded shortcut control is hidden' }
 Capture $dialog 'about-expanded.png'
 Post ([AboutSmoke]::GetDlgItem($dialog, 1003)) 0xf5 0
 Start-Sleep -Milliseconds 80
 [void][AboutSmoke]::GetWindowRect($dialog, [ref]$expanded)
 if (($expanded.Bottom-$expanded.Top) -ne ($compact.Bottom-$compact.Top)) { throw 'Shortcut panel did not collapse' }
 foreach ($link in @(@(1001,'https://github.com/Lianzy-Baimiao/FeatherMD'), @(1002,'https://ifdian.net/a/lianzy'))) {
  # BM_CLICK exercises the native accessible button. Cancel before ShellExecute.
  Post ([AboutSmoke]::GetDlgItem($dialog, $link[0])) 0xf5 0
  $confirmation = WaitWindow '#32770' '打开外部链接'
  [void][AboutSmoke]::ShowWindow($confirmation, 0)
  # The message box can be found before its text control is filled in.
  for ($i = 0; $i -lt 100 -and -not [AboutSmoke]::Content($confirmation).Contains($link[1]); $i++) { Start-Sleep -Milliseconds 20 }
  if (-not [AboutSmoke]::Content($confirmation).Contains($link[1])) { throw 'Wrong confirmation destination' }
  $default = [AboutSmoke]::SendMessageW($confirmation, 0x400, [IntPtr]::Zero, [IntPtr]::Zero).ToInt64() -band 0xffff
  if ($default -ne 2) { throw 'External link confirmation must default to Cancel' }
  Post $confirmation 0x111 2
  WaitClosed $confirmation; $confirmation = [IntPtr]::Zero
  if (-not [AboutSmoke]::IsWindow($dialog) -or -not [AboutSmoke]::IsWindowEnabled($dialog)) { throw 'Cancelling a link closed or disabled About' }
 }
 # Navigate from the safe default action to GitHub with real Tab messages, then Enter.
 $githubButton = [AboutSmoke]::GetDlgItem($dialog, 1001)
 for ($i=0; $i -lt 7; $i++) {
  if ([AboutSmoke]::Focused($dialog) -eq $githubButton) { break }
  Post $dialog 0x100 9; Start-Sleep -Milliseconds 40
 }
 if ([AboutSmoke]::Focused($dialog) -ne $githubButton) { throw 'Tab cannot reach GitHub' }
 Post $githubButton 0x100 13
 $confirmation = WaitWindow '#32770' '打开外部链接'
 Post $confirmation 0x111 2; WaitClosed $confirmation; $confirmation = [IntPtr]::Zero
 Capture $dialog 'about-keyboard-focus.png'
 Post ([AboutSmoke]::GetDlgItem($dialog, 2)) 0xf5 0
 WaitClosed $dialog; $dialog = [IntPtr]::Zero
 if (-not [AboutSmoke]::IsWindowEnabled($window)) { throw 'About did not re-enable reader' }
 # F1 remains an entry point; Escape/close must never launch a link.
 Post $window 0x100 0x70
 $dialog = WaitWindow '#32770' '关于 FeatherMD'
 [void][AboutSmoke]::ShowWindow($dialog, 0)
 Post $dialog 0x100 0x1b; WaitClosed $dialog; $dialog = [IntPtr]::Zero
 $gdiBefore = [AboutSmoke]::GetGuiResources($process.Handle, 0)
 for ($i=0; $i -lt 8; $i++) {
  Post $window 0x111 401; $dialog = WaitWindow '#32770' '关于 FeatherMD'
  [void][AboutSmoke]::ShowWindow($dialog, 0)
  Post $dialog 0x10 0; WaitClosed $dialog; $dialog = [IntPtr]::Zero
 }
 $gdiAfter = [AboutSmoke]::GetGuiResources($process.Handle, 0)
 if ($gdiAfter -gt $gdiBefore + 2) { throw "About leaked GDI resources: $gdiBefore -> $gdiAfter" }
 $metrics = [ordered]@{ result='PASS'; theme=$Theme; about_links=2; details_expand_collapse=$true; escape_closes=$true; tab_and_enter_open_confirmation=$true; gdi_before=$gdiBefore; gdi_after=$gdiAfter; destinations_confirmed=$true; cancel_is_default=$true; cancel_keeps_about_open=$true; reader_responsive_during_about=$true; f1_reopens=$true; browser_launched=$false; limitations='Native messages and in-process GDI snapshots; browser handoff success and real pointer input are not exercised.' }
 $metrics | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Output 'about.json') -Encoding utf8
 $metrics | ConvertTo-Json
} finally {
 if ($confirmation -ne [IntPtr]::Zero -and [AboutSmoke]::IsWindow($confirmation)) { Post $confirmation 0x111 2; Start-Sleep -Milliseconds 100 }
 if ($dialog -ne [IntPtr]::Zero -and [AboutSmoke]::IsWindow($dialog)) { Post $dialog 0x10 0; Start-Sleep -Milliseconds 100 }
 if ($process -and -not $process.HasExited) { Post $window 0x10 0; if (-not $process.WaitForExit(5000)) { $process.Kill() } }
 $env:FEATHERMD_TEST_SNAPSHOT = $oldSnapshot
 if ($createdIni) { Remove-Item -LiteralPath $ini -ErrorAction SilentlyContinue }
 if ($oldDpi -ne [IntPtr]::Zero) { [void][AboutSmoke]::SetThreadDpiAwarenessContext($oldDpi) }
}
