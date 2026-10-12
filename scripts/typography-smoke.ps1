param([ValidateSet('light','dark','sepia','system')][string]$Theme = 'light', [string]$Exe = "$PSScriptRoot\..\target\release\feathermd.exe", [string]$Output = "$PSScriptRoot\..\test-artifacts\typography")
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class FontSmoke {
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
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern bool SetWindowTextW(IntPtr hwnd, string text);
 [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")] public static extern IntPtr SendText(IntPtr hwnd, uint msg, IntPtr wp, string text);
 [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")] static extern IntPtr ReadText(IntPtr hwnd, uint msg, IntPtr wp, StringBuilder text);
 public static string Text(IntPtr hwnd) { var s = new StringBuilder(4096); ReadText(hwnd, 0x000d, (IntPtr)s.Capacity, s); return s.ToString(); }
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
$snapshot = Join-Path $Output 'typography.bmp'
$oldSnapshot = $env:FEATHERMD_TEST_SNAPSHOT
$createdIni = $false
$oldDpi = [FontSmoke]::SetThreadDpiAwarenessContext([IntPtr](-4))
function WaitWindow([string]$class, [string]$title) {
 for ($i = 0; $i -lt 100; $i++) {
  if ($process.HasExited) { throw 'Reader exited unexpectedly' }
  $h = [FontSmoke]::Find($process.Id, $class, $title)
  if ($h -ne [IntPtr]::Zero) { return $h }
  Start-Sleep -Milliseconds 30
 }
 throw "Window not found: $class / $title"
}
function Post([IntPtr]$h, [uint32]$message, [int]$value) {
 if (-not [FontSmoke]::PostMessageW($h, $message, [IntPtr]$value, [IntPtr]::Zero)) { throw 'Posting native message failed' }
}
function Capture([IntPtr]$h, [string]$name) {
 if ([FontSmoke]::SendMessageW($h, 0x802a, [IntPtr]4, [IntPtr]::Zero).ToInt64() -ne 1) { throw 'Font internal capture failed' }
 $bitmap = [Drawing.Bitmap]::new($snapshot)
 try {
  $color = $bitmap.GetPixel(4,4)
  $expected = switch ($Theme) { 'light' { '252,252,250' }; 'dark' { '25,29,34' }; 'sepia' { '247,239,221' }; default { $null } }
  if ($h -eq $dialog -and $expected -and "$($color.R),$($color.G),$($color.B)" -ne $expected) { throw "Wrong rendered theme / blank capture: $color" }
  $bitmap.Save((Join-Path $Output $name), [Drawing.Imaging.ImageFormat]::Png)
 } finally { $bitmap.Dispose() }
}
function WaitClosed([IntPtr]$h) {
 for ($i = 0; $i -lt 100; $i++) { if (-not [FontSmoke]::IsWindow($h)) { return }; Start-Sleep -Milliseconds 20 }
 throw 'Dialog failed to close'
}
function OpenFonts {
 Post $window 0x111 414
 $script:dialog = WaitWindow '#32770' '字体与默认字号'
 for($j=0;$j -lt 100;$j++) { if([FontSmoke]::GetDlgItem($dialog,1101) -ne [IntPtr]::Zero){break};Start-Sleep -Milliseconds 20 }
 [void][FontSmoke]::ShowWindow($dialog,0)
 # A synchronous message flushes initialization before driving controls.
 [void][FontSmoke]::SendMessageW($dialog,0,[IntPtr]::Zero,[IntPtr]::Zero)
}
function ChooseFont([string]$family,[int]$size) {
 $combo=[FontSmoke]::GetDlgItem($dialog,1101)
 $index=[FontSmoke]::SendText($combo,0x14d,[IntPtr](-1),$family).ToInt64()
 if($index -lt 0){throw "Required installed font not found: $family"}
 [void][FontSmoke]::SendMessageW($dialog,0x111,[IntPtr](1101 -bor (1 -shl 16)),$combo)
 [void][FontSmoke]::SendText([FontSmoke]::GetDlgItem($dialog,1102),0x000c,[IntPtr]::Zero,"$size")
}
function ReadSize { [FontSmoke]::SendMessageW($window,0x802a,[IntPtr]12,[IntPtr]::Zero).ToInt64() }
function StableGdi {
 $min=[int]::MaxValue
 for($j=0;$j -lt 12;$j++){Start-Sleep -Milliseconds 5;$min=[Math]::Min($min,[FontSmoke]::GetGuiResources($process.Handle,0))}
 return $min
}
try {
 $env:FEATHERMD_TEST_SNAPSHOT=$snapshot
 [IO.File]::WriteAllText($ini,"theme=$Theme
font_family=Microsoft YaHei UI
font_size=18
",[Text.UTF8Encoding]::new($false));$createdIni=$true
 $doc=(Resolve-Path "$PSScriptRoot\..\docs\demo.md").Path
 $process=Start-Process -FilePath $Exe -ArgumentList @('"'+$doc+'"') -WindowStyle Hidden -PassThru
 $window=WaitWindow 'FeatherMD.Native.Reader' $null
 Start-Sleep -Milliseconds 250
 Capture $window 'reading-before.png'
 OpenFonts
 if([FontSmoke]::IsWindowEnabled($window)){throw 'Font dialog is not modal'}
 if((ReadSize) -ne 18){throw 'Reader is blocked during modal loop'}
 $count=[FontSmoke]::SendMessageW([FontSmoke]::GetDlgItem($dialog,1101),0x146,[IntPtr]::Zero,[IntPtr]::Zero).ToInt64()
 if($count -lt 2){throw 'Installed fonts not enumerated'}
 Capture $dialog 'typography-default.png'
 $combo=[FontSmoke]::GetDlgItem($dialog,1101);$beforeFamily=[FontSmoke]::Text($combo)
 Post $combo 0x100 40;Start-Sleep -Milliseconds 40
 if([FontSmoke]::Text($combo) -eq $beforeFamily){throw 'Keyboard cannot change installed font'}

 foreach($bad in @(0,11,37,999)) {
  ChooseFont 'Segoe UI' $bad
  if([FontSmoke]::IsWindowEnabled([FontSmoke]::GetDlgItem($dialog,1))){throw "Invalid size enabled Save: $bad"}
  Post $dialog 0x111 1;Start-Sleep -Milliseconds 25
  if(-not [FontSmoke]::IsWindow($dialog)){throw 'Invalid size saved'}
 }
 ChooseFont 'Georgia' 24
 Capture $dialog 'typography-preview.png'
 # Enter on Cancel must cancel, not trigger the default Save command.
 Post ([FontSmoke]::GetDlgItem($dialog,2)) 0x100 13
 WaitClosed $dialog;$dialog=[IntPtr]::Zero
 if((ReadSize) -ne 18 -or (Get-Content -LiteralPath $ini -Raw) -notmatch 'font_family=Microsoft YaHei UI'){throw 'Cancel changed settings'}
 OpenFonts;ChooseFont 'Georgia' 22
 Post ([FontSmoke]::GetDlgItem($dialog,1102)) 0x100 13
 WaitClosed $dialog;$dialog=[IntPtr]::Zero
 Start-Sleep -Milliseconds 150
 if((ReadSize) -ne 22){throw 'Saved size not applied'}
 $saved=Get-Content -LiteralPath $ini -Raw
 if($saved -notmatch 'font_family=Georgia' -or $saved -notmatch 'font_size=22'){throw 'Font preferences not persisted'}
 Capture $window 'reading-custom.png'
 OpenFonts
 if([FontSmoke]::Text([FontSmoke]::GetDlgItem($dialog,1101)) -ne 'Georgia' -or [FontSmoke]::Text([FontSmoke]::GetDlgItem($dialog,1102)) -ne '22'){throw 'Saved draft not restored'}
 Post ([FontSmoke]::GetDlgItem($dialog,1103)) 0x100 13
 Start-Sleep -Milliseconds 40
 if([FontSmoke]::Text([FontSmoke]::GetDlgItem($dialog,1101)) -ne 'Microsoft YaHei UI'){throw 'Reset draft failed'}
 Post $dialog 0x100 27;WaitClosed $dialog;$dialog=[IntPtr]::Zero
 if((ReadSize) -ne 22){throw 'Escape saved reset draft'}
 $gdiBefore=StableGdi
 for($j=0;$j -lt 12;$j++) {
  OpenFonts;ChooseFont 'Segoe UI' (12+$j*2);Capture $dialog 'typography-stress.png'
  Post $dialog 0x10 0;WaitClosed $dialog;$dialog=[IntPtr]::Zero
 }
 $gdiAfter=StableGdi
 if($gdiAfter -gt $gdiBefore+2){throw "Font dialog leaked GDI handles: $gdiBefore -> $gdiAfter"}
 # Font family must not change chrome or monospace source rendering.
 Post $window 0x111 410;Start-Sleep -Milliseconds 120;Capture $window 'source-georgia.png'
 OpenFonts;ChooseFont 'Segoe UI' 22;Post $dialog 0x111 1;WaitClosed $dialog;$dialog=[IntPtr]::Zero
 Start-Sleep -Milliseconds 120;Capture $window 'source-segoe.png'
 $a=[Drawing.Bitmap]::new((Join-Path $Output 'source-georgia.png'));$b=[Drawing.Bitmap]::new((Join-Path $Output 'source-segoe.png'))
 try {
  $changed=0
  for($y=0;$y -lt $a.Height;$y++){for($x=0;$x -lt $a.Width;$x++){if($a.GetPixel($x,$y).ToArgb() -ne $b.GetPixel($x,$y).ToArgb()){$changed++}}}
  if($changed -ne 0){throw "Font choice changed source/chrome pixels: $changed"}
 } finally {$a.Dispose();$b.Dispose()}
 Post $window 0x111 409;Start-Sleep -Milliseconds 120;Capture $window 'reading-segoe.png'
 $a=[Drawing.Bitmap]::new((Join-Path $Output 'reading-custom.png'));$b=[Drawing.Bitmap]::new((Join-Path $Output 'reading-segoe.png'))
 try {
  $headerChanged=0;$bodyChanged=0;$dpi=[FontSmoke]::SendMessageW($window,0x802a,[IntPtr]3,[IntPtr]::Zero).ToInt64();$header=[int](48*$dpi/96)
  for($y=0;$y -lt $a.Height;$y++){for($x=0;$x -lt $a.Width;$x++){if($a.GetPixel($x,$y).ToArgb() -ne $b.GetPixel($x,$y).ToArgb()){if($y -lt $header){$headerChanged++}else{$bodyChanged++}}}}
  if($headerChanged -ne 0 -or $bodyChanged -lt 100){throw "Font application mismatch: header=$headerChanged body=$bodyChanged"}
 } finally {$a.Dispose();$b.Dispose()}
 Post $window 0x10 0;if(-not $process.WaitForExit(5000)){throw 'Reader failed to close'}
 $process=Start-Process -FilePath $Exe -ArgumentList @('"'+$doc+'"') -WindowStyle Hidden -PassThru
 $window=WaitWindow 'FeatherMD.Native.Reader' $null
 if((ReadSize) -ne 22){throw 'Size not restored on restart'}
 OpenFonts
 if([FontSmoke]::Text([FontSmoke]::GetDlgItem($dialog,1101)) -ne 'Segoe UI'){throw 'Font not restored on restart'}
 Post $dialog 0x100 27;WaitClosed $dialog;$dialog=[IntPtr]::Zero
 $metrics=[ordered]@{result='PASS';theme=$Theme;installed_fonts=$count;preview=$true;keyboard_changes_font=$true;body_font_changes_rendering=$true;invalid_sizes_rejected=4;save_cancel_reset_restart=$true;source_and_chrome_unchanged=$true;reader_responsive_during_dialog=$true;gdi_before=$gdiBefore;gdi_after=$gdiAfter;limitations='Native messages/in-process snapshots, not physical pointer or multi-monitor testing. Missing glyphs use Windows font fallback.'}
 $metrics|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $Output 'typography.json') -Encoding utf8
 $metrics|ConvertTo-Json
} finally {
 if($dialog -ne [IntPtr]::Zero -and [FontSmoke]::IsWindow($dialog)){Post $dialog 0x10 0;Start-Sleep -Milliseconds 100}
 if($process -and -not $process.HasExited){Post $window 0x10 0;if(-not $process.WaitForExit(5000)){$process.Kill()}}
 $env:FEATHERMD_TEST_SNAPSHOT=$oldSnapshot
 if($createdIni){Remove-Item -LiteralPath $ini -ErrorAction SilentlyContinue}
 if($oldDpi -ne [IntPtr]::Zero){[void][FontSmoke]::SetThreadDpiAwarenessContext($oldDpi)}
}
