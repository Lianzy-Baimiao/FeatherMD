param([ValidateSet('light','dark','sepia','system')][string]$Theme = 'light', [string]$Exe = "$PSScriptRoot\..\target\release\feathermd.exe", [string]$Output = "$PSScriptRoot\..\test-artifacts\settings", [switch]$AllowDesktopUi, [switch]$SecondaryMonitor)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/require-ui-test-host.ps1" -AllowDesktopUi:$AllowDesktopUi -SecondaryMonitor:$SecondaryMonitor -Exe $Exe
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class SettingsSmoke {
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
 [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd, int command);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
 [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")] public static extern IntPtr SendText(IntPtr hwnd, uint msg, IntPtr wp, string text);
 [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")] static extern IntPtr ReadText(IntPtr hwnd, uint msg, IntPtr wp, StringBuilder text);
 public static string Text(IntPtr hwnd) { var s = new StringBuilder(4096); ReadText(hwnd, 0x000d, (IntPtr)s.Capacity, s); return s.ToString(); }
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
$process = $null; $window = [IntPtr]::Zero; $dialog = [IntPtr]::Zero
$snapshot = Join-Path $Output 'settings.bmp'
$oldSnapshot = $env:FEATHERMD_TEST_SNAPSHOT
$createdIni = $false
$oldDpi = [SettingsSmoke]::SetThreadDpiAwarenessContext([IntPtr](-4))
# Gallery order (card ids 1120+): light skins, then dark ones. Queries 39/47 report Theme::ALL indices.
$skins = @('跟随系统','浅色','素白','雾灰','天青','护眼','书本','古籍','报纸','豆沙绿','樱粉','丁香','深色','暮色','极夜','夜蓝','深海','紫夜','咖啡','纯黑')
$themeIndex = @{ system = 0; light = 1; dark = 2; sepia = 3 }[$Theme]
function Query([int]$id) { [SettingsSmoke]::SendMessageW($window, 0x802a, [IntPtr]$id, [IntPtr]::Zero).ToInt64() }
function Post([IntPtr]$h, [uint32]$message, [int]$value) {
 if (-not [SettingsSmoke]::PostMessageW($h, $message, [IntPtr]$value, [IntPtr]::Zero)) { throw 'Posting native message failed' }
}
function Item([int]$id) {
 $h = [SettingsSmoke]::GetDlgItem($dialog, $id)
 if ($h -eq [IntPtr]::Zero) { throw "Settings control $id missing" }
 return $h
}
function WaitWindow([string]$class, [string]$title) {
 for ($i = 0; $i -lt 200; $i++) {
  if ($process.HasExited) { throw 'Reader exited unexpectedly' }
  $h = [SettingsSmoke]::Find($process.Id, $class, $title)
  if ($h -ne [IntPtr]::Zero) { return $h }
  Start-Sleep -Milliseconds 25
 }
 throw "Window not found: $class / $title"
}
function WaitClosed {
 for ($i = 0; $i -lt 200; $i++) { if (-not [SettingsSmoke]::IsWindow($dialog)) { $script:dialog = [IntPtr]::Zero; return }; Start-Sleep -Milliseconds 20 }
 throw 'Settings failed to close'
}
function Flush { [void][SettingsSmoke]::SendMessageW($dialog, 0, [IntPtr]::Zero, [IntPtr]::Zero); Start-Sleep -Milliseconds 30 }
function OpenSettings([int]$command) {
 Post $window 0x111 $command
 $script:dialog = WaitWindow '#32770' '设置'
 for ($j = 0; $j -lt 100 -and [SettingsSmoke]::GetDlgItem($dialog, 1103) -eq [IntPtr]::Zero; $j++) { Start-Sleep -Milliseconds 20 }
 [void][SettingsSmoke]::ShowWindow($dialog, 0)
 Flush
}
function Capture([IntPtr]$h, [string]$name, [string]$expected = '') {
 if ([SettingsSmoke]::SendMessageW($h, 0x802a, [IntPtr]4, [IntPtr]::Zero).ToInt64() -ne 1) { throw "Internal capture failed: $name" }
 $bitmap = [Drawing.Bitmap]::new($snapshot)
 try {
  # The content column (top right) uses the skin's page colour; blank captures fail here.
  $color = $bitmap.GetPixel($bitmap.Width - 6, 6)
  if ($expected -and "$($color.R),$($color.G),$($color.B)" -ne $expected) { throw "Wrong rendered skin in $name`: $color, expected $expected" }
  $bitmap.Save((Join-Path $Output $name), [Drawing.Imaging.ImageFormat]::Png)
 } finally { $bitmap.Dispose() }
}
function Layered { ([SettingsSmoke]::GetWindowLongW($window, -20) -band 0x80000) -ne 0 }
function StableGdi {
 $min = [int]::MaxValue
 for ($j = 0; $j -lt 12; $j++) { Start-Sleep -Milliseconds 5; $min = [Math]::Min($min, [SettingsSmoke]::GetGuiResources($process.Handle, 0)) }
 return $min
}
function StartReader {
 $script:process = Start-Process -FilePath $Exe -ArgumentList @('"' + $doc + '"') -WindowStyle Hidden -PassThru
 $script:window = WaitWindow 'FeatherMD.Native.Reader' $null
 for ($j = 0; $j -lt 300 -and ((Query 2) -eq 0 -or (Query 11) -ne 0); $j++) { Start-Sleep -Milliseconds 10 }
}
try {
 $env:FEATHERMD_TEST_SNAPSHOT = $snapshot
 [IO.File]::WriteAllText($ini, "theme=$Theme`nfont_size=18`nlanguage=zh-CN`n", [Text.UTF8Encoding]::new($false)); $createdIni = $true
 $doc = (Resolve-Path "$PSScriptRoot\..\docs\demo.md").Path
 StartReader
 $page = @{ light = '252,252,250'; dark = '25,29,34'; sepia = '247,239,221' }[$Theme]

 # Appearance page: twenty named skin cards, opened from More → Settings.
 OpenSettings 415
 if ([SettingsSmoke]::IsWindowEnabled($window)) { throw 'Settings is not modal' }
 if ((Query 12) -ne 18) { throw 'Reader is blocked during the Settings modal loop' }
 for ($i = 0; $i -lt $skins.Count; $i++) {
  if ([SettingsSmoke]::Text((Item (1120 + $i))) -ne $skins[$i]) { throw "Skin card $i is not $($skins[$i])" }
 }
 if (([SettingsSmoke]::GetWindowLongW($dialog, -20) -band 0x80000) -ne 0) { throw 'Settings window must stay opaque' }
 Capture $dialog "appearance-$Theme.png" $page

 # Clicking a card previews it on the reader without saving (card 7 is Ancient, ALL index 5).
 Post (Item 1127) 0xf5 0; Flush
 if ((Query 47) -ne 5 -or (Query 39) -ne $themeIndex) { throw "Ancient preview failed: preview=$(Query 47) saved=$(Query 39)" }
 Capture $dialog 'appearance-ancient.png' '236,224,197'
 Capture $window 'preview-ancient.png'
 # Native trackbar keyboard path: four Left presses lower opacity by 20%.
 for ($i = 0; $i -lt 4; $i++) { Post (Item 1140) 0x100 0x25; Flush }
 if ([SettingsSmoke]::Text((Item 1141)) -ne '80%') { throw "Opacity label: $([SettingsSmoke]::Text((Item 1141)))" }
 if ((Query 48) -ne 204 -or -not (Layered)) { throw "Opacity preview not applied: alpha=$(Query 48)" }
 Capture $dialog 'appearance-opacity.png'
 Post $dialog 0x111 2; WaitClosed
 if ((Query 47) -ne -1 -or (Query 39) -ne $themeIndex -or (Query 48) -ne 255 -or (Layered)) { throw 'Cancel did not restore skin and opacity' }
 if ((Get-Content -LiteralPath $ini -Raw) -match 'opacity=80|theme=ancient') { throw 'Cancel saved the draft' }

 # Save a skin and opacity (card 6 is Book, ALL index 4).
 OpenSettings 415
 Post (Item 1126) 0xf5 0; Flush
 for ($i = 0; $i -lt 5; $i++) { Post (Item 1140) 0x100 0x25; Flush }
 Post $dialog 0x111 1; WaitClosed
 Start-Sleep -Milliseconds 80
 if ((Query 39) -ne 4 -or (Query 47) -ne -1 -or (Query 48) -ne 191) { throw "Book / 75% not applied: theme=$(Query 39) alpha=$(Query 48)" }
 $saved = Get-Content -LiteralPath $ini -Raw
 if ($saved -notmatch '(?m)^theme=book\r?$' -or $saved -notmatch '(?m)^opacity=75\r?$') { throw 'Skin/opacity not persisted' }
 Capture $window 'reading-book.png'

 # Arrow keys move through the 5x4 grid and preview; Escape cancels.
 OpenSettings 415
 Post (Item 1126) 0x100 0x27; Flush
 if ((Query 47) -ne 5) { throw "Right arrow did not select Ancient: $(Query 47)" }
 Post (Item 1127) 0x100 0x28; Flush
 if ((Query 47) -ne 2) { throw "Down arrow did not select Dark: $(Query 47)" }
 Post (Item 1132) 0x100 0x25; Flush
 if ((Query 47) -ne 14) { throw "Left arrow did not select Lilac: $(Query 47)" }
 Post (Item 1131) 0x100 0x26; Flush
 if ((Query 47) -ne 4) { throw "Up arrow did not select Book: $(Query 47)" }
 # Restore defaults resets only this page's draft.
 Post (Item 1103) 0xf5 0; Flush
 if ((Query 47) -ne 0 -or (Query 48) -ne 255) { throw 'Appearance defaults did not reset skin/opacity' }
 Post $dialog 0x100 27; WaitClosed
 if ((Query 39) -ne 4 -or (Query 48) -ne 191 -or (Query 47) -ne -1) { throw 'Escape kept the arrow-key draft' }

 # The header picker's fourth row opens the gallery on the appearance page.
 [void][SettingsSmoke]::SendMessageW($window, 0x111, [IntPtr]412, [IntPtr]::Zero)
 if ((Query 42) -ne 3) { throw "Picker did not focus the gallery row for a gallery skin: $(Query 42)" }
 [void][SettingsSmoke]::SendMessageW($window, 0x100, [IntPtr]13, [IntPtr]::Zero)
 $script:dialog = WaitWindow '#32770' '设置'
 [void][SettingsSmoke]::ShowWindow($dialog, 0); Flush
 if (([SettingsSmoke]::GetWindowLongW((Item 1124), -16) -band 0x10000000) -eq 0) { throw 'More skins did not open the appearance page' }
 if (([SettingsSmoke]::GetWindowLongW((Item 1101), -16) -band 0x10000000) -ne 0) { throw 'Reading controls visible on the appearance page' }
 Post $dialog 0x100 27; WaitClosed

 # Editor page: choose Notepad through the native combo and save.
 OpenSettings 416
 $editors = Item 1150
 if ([SettingsSmoke]::SendMessageW($editors, 0x146, [IntPtr]::Zero, [IntPtr]::Zero).ToInt64() -lt 2) { throw 'Editor list lacks system/Notepad entries' }
 if ([SettingsSmoke]::SendText($editors, 0x14d, [IntPtr](-1), '记事本').ToInt64() -lt 0) { throw 'Notepad entry missing' }
 [void][SettingsSmoke]::SendMessageW($dialog, 0x111, [IntPtr](1150 -bor (1 -shl 16)), $editors)
 if ([SettingsSmoke]::Text((Item 1152)) -notmatch 'notepad\.exe$') { throw "Editor path not shown: $([SettingsSmoke]::Text((Item 1152)))" }
 Capture $dialog 'editor.png'
 Post $dialog 0x111 1; WaitClosed
 if ((Get-Content -LiteralPath $ini -Raw) -notmatch '(?m)^editor=notepad\r?$') { throw 'Editor choice not persisted' }

 # Reading page keeps the former font dialog behaviour, plus "Show images", off by default.
 OpenSettings 414
 Capture $dialog 'reading.png'
 if ([SettingsSmoke]::SendMessageW((Item 1105), 0xf0, [IntPtr]::Zero, [IntPtr]::Zero).ToInt64() -ne 0) { throw 'Show images is checked by default' }
 Post (Item 1105) 0xf5 0; Flush
 if ([SettingsSmoke]::SendMessageW((Item 1105), 0xf0, [IntPtr]::Zero, [IntPtr]::Zero).ToInt64() -ne 1) { throw 'Show images did not check' }
 Capture $dialog 'reading-images.png'
 Post $dialog 0x111 1; WaitClosed
 if ((Get-Content -LiteralPath $ini -Raw) -notmatch '(?m)^images=true\r?$') { throw 'Show images not persisted' }
 OpenSettings 414
 if ([SettingsSmoke]::SendMessageW((Item 1105), 0xf0, [IntPtr]::Zero, [IntPtr]::Zero).ToInt64() -ne 1) { throw 'Show images not restored' }
 # Restore defaults on this page clears it again.
 Post (Item 1103) 0xf5 0; Flush
 if ([SettingsSmoke]::SendMessageW((Item 1105), 0xf0, [IntPtr]::Zero, [IntPtr]::Zero).ToInt64() -ne 0) { throw 'Reading defaults kept Show images' }
 Post $dialog 0x111 1; WaitClosed
 if ((Get-Content -LiteralPath $ini -Raw) -notmatch '(?m)^images=false\r?$') { throw 'Show images default not saved' }

 $gdiBefore = StableGdi
 for ($i = 0; $i -lt 12; $i++) {
  OpenSettings (@(415, 414, 416)[$i % 3]); Capture $dialog 'stress.png'
  Post $dialog 0x10 0; WaitClosed
 }
 $gdiAfter = StableGdi
 if ($gdiAfter -gt $gdiBefore + 2) { throw "Settings leaked GDI handles: $gdiBefore -> $gdiAfter" }

 # Restart restores the saved skin and opacity.
 Post $window 0x10 0; if (-not $process.WaitForExit(5000)) { throw 'Reader failed to close' }
 StartReader
 if ((Query 39) -ne 4 -or (Query 48) -ne 191 -or -not (Layered)) { throw 'Skin/opacity not restored on restart' }
 Capture $window 'reading-book-restart.png'
 $metrics = [ordered]@{
  result = 'PASS'; theme = $Theme; skins = $skins.Count; live_preview_and_cancel = $true; opacity_keyboard_and_alpha = $true
  arrow_grid_navigation = $true; page_defaults = $true; picker_gallery_row = $true; editor_choice_persisted = $true
  show_images_default_off_saved_and_reset = $true
  save_and_restart = $true; gdi_before = $gdiBefore; gdi_after = $gdiAfter
  limitations = 'Hidden windows and native messages; no physical pointer, real desktop compositing or multi-monitor DPI checks.'
 }
 $metrics | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Output 'settings.json') -Encoding utf8
 $metrics | ConvertTo-Json
} finally {
 if ($dialog -ne [IntPtr]::Zero -and [SettingsSmoke]::IsWindow($dialog)) { Post $dialog 0x10 0; Start-Sleep -Milliseconds 100 }
 if ($process -and -not $process.HasExited) { Post $window 0x10 0; if (-not $process.WaitForExit(5000)) { $process.Kill() } }
 $env:FEATHERMD_TEST_SNAPSHOT = $oldSnapshot
 if ($createdIni) { Remove-Item -LiteralPath $ini -ErrorAction SilentlyContinue }
 if ($oldDpi -ne [IntPtr]::Zero) { [void][SettingsSmoke]::SetThreadDpiAwarenessContext($oldDpi) }
}

. "$PSScriptRoot/verify-ui-test-host.ps1" -Output $Output -SecondaryMonitor:$SecondaryMonitor
