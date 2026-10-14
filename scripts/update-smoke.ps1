param([ValidateSet('light','dark')][string]$Theme = 'light', [string]$Exe = "$PSScriptRoot\..\target\release\feathermd.exe", [string]$Server = "$PSScriptRoot\..\target\release\examples\update_server.exe", [string]$Output = "$PSScriptRoot\..\test-artifacts\update", [switch]$AllowNoPopup)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class UpdateSmoke {
 public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr data);
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr hwnd, EnumProc proc, IntPtr data);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr hwnd, StringBuilder text, int size);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] public static extern int GetWindowLongW(IntPtr hwnd, int index);
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr hwnd, int id);
 [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process, uint flag);
 [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd, int command);
 [DllImport("user32.dll")] public static extern int GetMenuItemCount(IntPtr menu);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetMenuStringW(IntPtr menu, uint item, StringBuilder text, int max, uint flags);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
 [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")] static extern IntPtr ReadText(IntPtr hwnd, uint msg, IntPtr wp, StringBuilder text);
 public static string Text(IntPtr hwnd) { var s = new StringBuilder(4096); ReadText(hwnd, 0x000d, (IntPtr)s.Capacity, s); return s.ToString(); }
 public static string MenuText(IntPtr menu, uint position) { var s = new StringBuilder(512); GetMenuStringW(menu, position, s, s.Capacity, 0x400); return s.ToString(); }
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
if (-not (Test-Path -LiteralPath $Server)) { throw 'Build the stand-in first: cargo build --release --example update_server' }
$Server = (Resolve-Path -LiteralPath $Server).Path
New-Item -ItemType Directory -Path $Output -Force | Out-Null
$Output = (Resolve-Path -LiteralPath $Output).Path
$ini = Join-Path (Split-Path $Exe) 'FeatherMD.ini'
if (Test-Path -LiteralPath $ini) { throw 'Refusing to overwrite existing portable settings' }
$feed = Join-Path $Output 'feed'
Remove-Item -LiteralPath $feed -Recurse -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $feed -Force | Out-Null
$version = [Diagnostics.FileVersionInfo]::GetVersionInfo($Exe).ProductVersion.Trim()
$published = [DateTime]::UtcNow.ToString('yyyy-MM-dd')
$snapshot = Join-Path $Output 'update.bmp'
$oldSnapshot = $env:FEATHERMD_TEST_SNAPSHOT; $oldUrl = $env:FEATHERMD_TEST_UPDATE_URL
$process = $null; $window = [IntPtr]::Zero; $dialog = [IntPtr]::Zero; $serverProcess = $null; $createdIni = $false
$oldDpi = [UpdateSmoke]::SetThreadDpiAwarenessContext([IntPtr](-4))
$accent = @{ light = '20,113,99'; dark = '112,211,181' }[$Theme]
$page = @{ light = '252,252,250'; dark = '25,29,34' }[$Theme]
function Query([int]$id) { [UpdateSmoke]::SendMessageW($window, 0x802a, [IntPtr]$id, [IntPtr]::Zero).ToInt64() }
function Post([IntPtr]$h, [uint32]$message, [int]$value) { if (-not [UpdateSmoke]::PostMessageW($h, $message, [IntPtr]$value, [IntPtr]::Zero)) { throw 'Posting native message failed' } }
function Item([int]$id) { $h = [UpdateSmoke]::GetDlgItem($dialog, $id); if ($h -eq [IntPtr]::Zero) { throw "Settings control $id missing" }; return $h }
function Shown([int]$id) { ([UpdateSmoke]::GetWindowLongW((Item $id), -16) -band 0x10000000) -ne 0 }
function Release([string]$tag, [string]$status = '200 OK', [int]$delay = 0) {
 [IO.File]::WriteAllText((Join-Path $feed 'status'), $status)
 [IO.File]::WriteAllText((Join-Path $feed 'delay'), "$delay")
 $json = '{"url":"https://api.github.com/repos/Lianzy-Baimiao/FeatherMD/releases/1","author":{"login":"lianzy","tag_name":"v0.0.1"},"tag_name":"' + $tag + '","name":"FeatherMD 羽读 ' + $tag + '","draft":false,"prerelease":false,"published_at":"' + $published + 'T03:04:05Z","assets":[{"name":"FeatherMD.exe","size":1}],"body":"notes"}'
 [IO.File]::WriteAllText((Join-Path $feed 'body'), $json, [Text.UTF8Encoding]::new($false))
}
function Requests { $log = Join-Path $feed 'requests.log'; if (Test-Path -LiteralPath $log) { @([IO.File]::ReadAllLines($log) | Where-Object { $_.StartsWith('GET ') }).Count } else { 0 } }
function WaitWindow([string]$class, [string]$title) {
 for ($i = 0; $i -lt 200; $i++) {
  if ($process.HasExited) { throw 'Reader exited unexpectedly' }
  $h = [UpdateSmoke]::Find($process.Id, $class, $title)
  if ($h -ne [IntPtr]::Zero) { return $h }
  Start-Sleep -Milliseconds 25
 }
 throw "Window not found: $class / $title"
}
function Flush { [void][UpdateSmoke]::SendMessageW($dialog, 0, [IntPtr]::Zero, [IntPtr]::Zero) }
function WaitClosed { for ($i = 0; $i -lt 200; $i++) { if (-not [UpdateSmoke]::IsWindow($dialog)) { $script:dialog = [IntPtr]::Zero; return }; Start-Sleep -Milliseconds 20 }; throw 'Settings failed to close' }
function OpenUpdates([int]$command) {
 Post $window 0x111 $command
 $script:dialog = WaitWindow '#32770' '设置'
 for ($j = 0; $j -lt 100 -and [UpdateSmoke]::GetDlgItem($dialog, 1161) -eq [IntPtr]::Zero; $j++) { Start-Sleep -Milliseconds 20 }
 [void][UpdateSmoke]::ShowWindow($dialog, 0)
 Flush
}
function WaitStatus([string]$pattern) {
 for ($i = 0; $i -lt 400; $i++) {
  $status = [UpdateSmoke]::Text((Item 1163))
  if ($status -match $pattern) { return $status }
  Start-Sleep -Milliseconds 25
 }
 throw "Update status '$status' never matched $pattern"
}
function Capture([IntPtr]$h, [string]$name) {
 if ([UpdateSmoke]::SendMessageW($h, 0x802a, [IntPtr]4, [IntPtr]::Zero).ToInt64() -ne 1) { throw "Capture failed: $name" }
 $bitmap = [Drawing.Bitmap]::new($snapshot)
 try { $bitmap.Save((Join-Path $Output $name), [Drawing.Imaging.ImageFormat]::Png); return $bitmap.Clone() } finally { $bitmap.Dispose() }
}
function Badge {
 # Any accent pixel in the top-right corner of the More button (header_left + 124..140, 4..18 logical px).
 $dpi = Query 3
 $left = [int](Query 0) - [Math]::Floor(132 * $dpi / 96) - [Math]::Floor(148 * $dpi / 96) - 1
 $image = Capture $window 'header.png'
 try {
  for ($y = [Math]::Floor(4 * $dpi / 96); $y -le [Math]::Floor(18 * $dpi / 96); $y++) {
   for ($x = $left + [Math]::Floor(124 * $dpi / 96); $x -le $left + [Math]::Floor(140 * $dpi / 96); $x++) {
    $c = $image.GetPixel($x, $y)
    if ("$($c.R),$($c.G),$($c.B)" -eq $accent) { return $accent }
   }
  }
  return 'none'
 } finally { $image.Dispose() }
}
function MoreMenuUpdateLabel {
 [void][UpdateSmoke]::PostMessageW($window, 0x111, [IntPtr]403, [IntPtr]::Zero)
 $menu = [IntPtr]::Zero
 for ($i = 0; $i -lt 200; $i++) {
  $popup = [UpdateSmoke]::Find($process.Id, '#32768', $null)
  if ($popup -ne [IntPtr]::Zero -and [UpdateSmoke]::IsWindowVisible($popup)) { $menu = [UpdateSmoke]::SendMessageW($popup, 0x1e1, [IntPtr]::Zero, [IntPtr]::Zero); break }
  Start-Sleep -Milliseconds 10
 }
 if ($menu -eq [IntPtr]::Zero) {
  [void][UpdateSmoke]::PostMessageW($window, 0x1f, [IntPtr]::Zero, [IntPtr]::Zero)
  if ($AllowNoPopup) { return 'NO_POPUP' }
  throw 'More menu did not appear'
 }
 $labels = @(0..([UpdateSmoke]::GetMenuItemCount($menu) - 1) | ForEach-Object { [UpdateSmoke]::MenuText($menu, $_) })
 [void][UpdateSmoke]::PostMessageW($popup, 0x100, [IntPtr]0x1b, [IntPtr]::Zero)
 for ($i = 0; $i -lt 100 -and [UpdateSmoke]::IsWindowVisible($popup); $i++) { Start-Sleep -Milliseconds 20 }
 $match = @($labels | Where-Object { $_ -match '检查更新|新版本' })
 if ($match.Count -ne 1) { throw "More menu update entries: $($labels -join ' | ')" }
 return $match[0]
}
function StartReader([string]$settings) {
 [IO.File]::WriteAllText($ini, $settings + "`nlanguage=zh-CN`n", [Text.UTF8Encoding]::new($false)); $script:createdIni = $true
 $script:process = Start-Process -FilePath $Exe -WindowStyle Hidden -PassThru
 $script:window = WaitWindow 'FeatherMD.Native.Reader' $null
 for ($j = 0; $j -lt 200 -and (Query 0) -le 0; $j++) { Start-Sleep -Milliseconds 10 }
}
function StopReader { if ($process -and -not $process.HasExited) { Post $window 0x10 0; if (-not $process.WaitForExit(5000)) { $process.Kill() } } }
function StableGdi { $min = [int]::MaxValue; for ($j = 0; $j -lt 12; $j++) { Start-Sleep -Milliseconds 5; $min = [Math]::Min($min, [UpdateSmoke]::GetGuiResources($process.Handle, 0)) }; return $min }
try {
 $env:FEATHERMD_TEST_SNAPSHOT = $snapshot
 Release 'v99.0.0'
 $serverProcess = Start-Process -FilePath $Server -ArgumentList @('"' + $feed + '"') -WindowStyle Hidden -PassThru
 for ($i = 0; $i -lt 200 -and -not (Test-Path -LiteralPath (Join-Path $feed 'port')); $i++) { Start-Sleep -Milliseconds 20 }
 $port = [IO.File]::ReadAllText((Join-Path $feed 'port')).Trim()
 $env:FEATHERMD_TEST_UPDATE_URL = "http://127.0.0.1:$port/repos/Lianzy-Baimiao/FeatherMD/releases/latest"

 # Default: no automatic check, no request, nothing recorded.
 StartReader "theme=$Theme`n"
 Start-Sleep -Milliseconds 4500
 if ((Query 51) -ne 0 -or (Requests) -ne 0) { throw 'An update check ran although it is off by default' }
 if ((Query 52) -ne 0 -or (Badge) -eq $accent) { throw 'Update badge shown without a check' }
 StopReader
 if ((Get-Content -LiteralPath $ini -Raw) -match 'update_latest=v') { throw 'Default launch recorded an update' }

 # Opted in and due: exactly one background request, a recorded tag and a header badge.
 StartReader "theme=$Theme`nupdate_check=true`nupdate_checked=0`n"
 if ((Query 51) -ne 0) { throw 'The startup check did not wait for first paint' }
 for ($i = 0; $i -lt 400 -and ((Query 52) -ne 1 -or (Query 53) -ne 0); $i++) { Start-Sleep -Milliseconds 25 }
 if ((Query 51) -ne 1 -or (Query 52) -ne 1 -or (Requests) -ne 1) { throw "Automatic check failed: started=$(Query 51) newer=$(Query 52) requests=$(Requests)" }
 $request = ([IO.File]::ReadAllText((Join-Path $feed 'requests.log'))).ToLowerInvariant()
 if (-not $request.StartsWith('get /repos/lianzy-baimiao/feathermd/releases/latest http/1.1')) { throw "Unexpected request line: $request" }
 foreach ($expected in @("user-agent: feathermd/$version", 'accept: application/vnd.github+json', 'x-github-api-version: 2022-11-28')) {
  if (-not $request.Contains($expected)) { throw "Request lacks $expected" }
 }
 foreach ($private in @('cookie:', 'authorization:', 'referer:')) { if ($request.Contains($private)) { throw "Request sent $private" } }
 $saved = Get-Content -LiteralPath $ini -Raw
 if ($saved -notmatch '(?m)^update_latest=v99\.0\.0\r?$' -or $saved -notmatch '(?m)^update_checked=[1-9]\d+\r?$') { throw 'Automatic result not recorded' }
 if ((Badge) -ne $accent) { throw "Header badge missing: $(Badge)" }
 Copy-Item -LiteralPath (Join-Path $Output 'header.png') -Destination (Join-Path $Output "header-badge-$Theme.png") -Force
 $label = MoreMenuUpdateLabel
 if ($label -ne 'NO_POPUP' -and $label -ne '发现新版本 v99.0.0…') { throw "More menu update label: $label" }
 StopReader

 # Restart within a day: the badge stays, no new request.
 $saved = Get-Content -LiteralPath $ini -Raw
 [IO.File]::WriteAllText($ini, $saved, [Text.UTF8Encoding]::new($false))
 $script:process = Start-Process -FilePath $Exe -WindowStyle Hidden -PassThru
 $script:window = WaitWindow 'FeatherMD.Native.Reader' $null
 Start-Sleep -Milliseconds 4500
 if ((Query 51) -ne 0 -or (Requests) -ne 1) { throw 'Checked again within the daily interval' }
 if ((Query 52) -ne 1 -or (Badge) -ne $accent) { throw 'Known update not shown after restart' }

 # Manual: More → Check for updates opens the page and checks immediately.
 Release "v$version"
 OpenUpdates 417
 if (-not (Shown 1161)) { throw 'Check for updates did not open the update page' }
 WaitStatus '^已是最新版本$' | Out-Null
 if ((Requests) -ne 2 -or -not [UpdateSmoke]::IsWindowEnabled((Item 1161)) -or (Shown 1162)) { throw 'Up-to-date check state is wrong' }
 for ($i = 0; $i -lt 100 -and (Query 52) -ne 0; $i++) { Start-Sleep -Milliseconds 20 }
 if ((Query 52) -ne 0) { throw 'Badge stayed after an up-to-date manual check' }
 if ([UpdateSmoke]::Text((Item 1165)) -ne '上次成功检查：刚刚') { throw "Last check text: $([UpdateSmoke]::Text((Item 1165)))" }
 Capture $dialog "update-latest-$Theme.png" | ForEach-Object { $_.Dispose() }

 # A newer release: accent status, detail, and a confirmed (here: cancelled) link.
 Release 'v99.1.0' '200 OK' 600
 Post (Item 1161) 0xf5 0
 WaitStatus '^正在检查更新…$' | Out-Null
 if ([UpdateSmoke]::IsWindowEnabled((Item 1161))) { throw 'Check now stayed enabled while checking' }
 WaitStatus '^发现新版本 v99\.1\.0$' | Out-Null
 if ([UpdateSmoke]::Text((Item 1164)) -notmatch ('^FeatherMD 羽读 v99\.1\.0　发布于 ' + $published + '$')) { throw "Release detail: $([UpdateSmoke]::Text((Item 1164)))" }
 if (-not (Shown 1162)) { throw 'Download button hidden for a newer release' }
 $image = Capture $dialog "update-available-$Theme.png"
 try { $c = $image.GetPixel($image.Width - 6, 6); if ("$($c.R),$($c.G),$($c.B)" -ne $page) { throw "Wrong dialog skin: $c" } } finally { $image.Dispose() }
 Post (Item 1162) 0xf5 0
 $confirm = WaitWindow '#32770' '打开外部链接'
 $destination = 'https://github.com/Lianzy-Baimiao/FeatherMD/releases/tag/v99.1.0'
 # The message box can be found before its text control is filled in.
 for ($i = 0; $i -lt 100 -and -not [UpdateSmoke]::Content($confirm).Contains($destination); $i++) { Start-Sleep -Milliseconds 20 }
 if (-not [UpdateSmoke]::Content($confirm).Contains($destination)) { throw 'Confirmation does not name the release page' }
 Post $confirm 0x111 2
 for ($i = 0; $i -lt 100 -and [UpdateSmoke]::IsWindow($confirm); $i++) { Start-Sleep -Milliseconds 20 }

 # Failures are explained and keep the last known result. Each check must pass
 # through "checking" so an identical previous status cannot satisfy the wait.
 Release 'v99.1.0' '403 Forbidden' 400
 Post (Item 1161) 0xf5 0
 WaitStatus '^正在检查更新…$' | Out-Null
 WaitStatus '^检查失败$' | Out-Null
 if ([UpdateSmoke]::Text((Item 1164)) -ne 'GitHub 暂时限制了访问频率，请稍后再试') { throw "403 detail: $([UpdateSmoke]::Text((Item 1164)))" }
 if (-not (Shown 1162) -or (Query 52) -ne 1) { throw 'A failed check discarded the known release' }
 [IO.File]::WriteAllText((Join-Path $feed 'status'), '200 OK')
 [IO.File]::WriteAllText((Join-Path $feed 'body'), '<!DOCTYPE html><title>proxy login</title>')
 Post (Item 1161) 0xf5 0
 WaitStatus '^正在检查更新…$' | Out-Null
 WaitStatus '^检查失败$' | Out-Null
 if ([UpdateSmoke]::Text((Item 1164)) -ne '更新信息格式无效') { throw "HTML detail: $([UpdateSmoke]::Text((Item 1164)))" }

 # The opt-in switch is a real checkbox and only changes on Save.
 if ([UpdateSmoke]::SendMessageW((Item 1160), 0xf0, [IntPtr]::Zero, [IntPtr]::Zero).ToInt64() -ne 1) { throw 'Auto-check box does not reflect the saved setting' }
 Post (Item 1160) 0xf5 0; Flush
 if ([UpdateSmoke]::SendMessageW((Item 1160), 0xf0, [IntPtr]::Zero, [IntPtr]::Zero).ToInt64() -ne 0) { throw 'Auto-check box did not toggle' }
 Capture $dialog "update-unchecked-$Theme.png" | ForEach-Object { $_.Dispose() }
 Post $dialog 0x111 2; WaitClosed
 if ((Get-Content -LiteralPath $ini -Raw) -notmatch '(?m)^update_check=true\r?$') { throw 'Cancel changed the auto-check setting' }
 OpenUpdates 415
 Post (Item 1113) 0xf5 0; Flush
 Post (Item 1160) 0xf5 0; Flush
 Post $dialog 0x111 1; WaitClosed
 if ((Get-Content -LiteralPath $ini -Raw) -notmatch '(?m)^update_check=false\r?$') { throw 'Saving did not turn automatic checks off' }

 # Closing during a check leaves the reader responsive; the late reply is ignored.
 Release 'v99.2.0' '200 OK' 1500
 OpenUpdates 417
 Post $dialog 0x100 27; WaitClosed
 Start-Sleep -Milliseconds 2000
 if ((Query 12) -le 0 -or (Query 53) -ne 0) { throw 'Reader unresponsive after closing a running check' }

 Release "v$version"
 $gdiBefore = StableGdi
 for ($i = 0; $i -lt 8; $i++) { OpenUpdates 417; WaitStatus '^已是最新版本$' | Out-Null; Post $dialog 0x10 0; WaitClosed }
 $gdiAfter = StableGdi
 if ($gdiAfter -gt $gdiBefore + 2) { throw "Update page leaked GDI handles: $gdiBefore -> $gdiAfter" }
 $metrics = [ordered]@{
  result = 'PASS'; theme = $Theme; author = 'lianzy'; default_off_no_request = $true; startup_check_once_daily = $true
  request_headers_public_only = $true; header_badge = $true; more_menu_label_checked = ($label -ne 'NO_POPUP'); manual_latest_newer_failure_html = $true
  link_requires_confirmation = $true; checkbox_saved_only_on_save = $true; close_during_check = $true
  requests = (Requests); gdi_before = $gdiBefore; gdi_after = $gdiAfter
  limitations = 'Loopback stand-in for GitHub; the live API, proxies and TLS are not exercised by this script.'
 }
 $metrics | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Output 'update.json') -Encoding utf8
 $metrics | ConvertTo-Json
} finally {
 if ($dialog -ne [IntPtr]::Zero -and [UpdateSmoke]::IsWindow($dialog)) { Post $dialog 0x10 0; Start-Sleep -Milliseconds 100 }
 StopReader
 if ($serverProcess -and -not $serverProcess.HasExited) {
  [IO.File]::WriteAllText((Join-Path $feed 'stop'), '1')
  if (-not $serverProcess.WaitForExit(3000)) { $serverProcess.Kill() }
 }
 $env:FEATHERMD_TEST_SNAPSHOT = $oldSnapshot; $env:FEATHERMD_TEST_UPDATE_URL = $oldUrl
 if ($createdIni) { Remove-Item -LiteralPath $ini -ErrorAction SilentlyContinue }
 if ($oldDpi -ne [IntPtr]::Zero) { [void][UpdateSmoke]::SetThreadDpiAwarenessContext($oldDpi) }
}
