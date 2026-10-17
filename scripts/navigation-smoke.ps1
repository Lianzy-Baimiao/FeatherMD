param([ValidateSet('headings','wrapped')][string]$Shape = 'headings', [string]$Exe = "$PSScriptRoot\..\target\release\feathermd.exe", [string]$Output = "$PSScriptRoot\..\test-artifacts\navigation", [switch]$AllowDesktopUi, [switch]$SecondaryMonitor)
$ErrorActionPreference='Stop'
. "$PSScriptRoot/require-ui-test-host.ps1" -AllowDesktopUi:$AllowDesktopUi -SecondaryMonitor:$SecondaryMonitor -Exe $Exe
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class NavigationSmoke {
 public delegate bool EnumProc(IntPtr hwnd,IntPtr data);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback,IntPtr data);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr hwnd,System.Text.StringBuilder text,int count);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint id);
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hwnd,out Rect r);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd,IntPtr dc,uint flags);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,EntryPoint="SendMessageW")] public static extern IntPtr SendText(IntPtr hwnd,uint msg,IntPtr wp,string text);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr hwnd,uint msg,IntPtr wp,IntPtr lp);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr hwnd,uint msg,IntPtr wp,IntPtr lp);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowExW(IntPtr parent,IntPtr after,string cls,string title);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern bool SetWindowTextW(IntPtr hwnd,string text);
 [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr process,uint flags);
 public struct Rect { public int Left,Top,Right,Bottom; }
 public static IntPtr Find(uint pid) {IntPtr found=IntPtr.Zero;EnumWindows((h,d)=>{uint p;GetWindowThreadProcessId(h,out p);var cls=new System.Text.StringBuilder(128);GetClassNameW(h,cls,128);if(p==pid&&cls.ToString()=="FeatherMD.Native.Reader"){found=h;return false;}return true;},IntPtr.Zero);return found;}
}
"@

New-Item -ItemType Directory -Path $Output -Force | Out-Null
$Output=(Resolve-Path -LiteralPath $Output).Path
$Exe=(Resolve-Path -LiteralPath $Exe).Path
$document=Join-Path $Output ($Shape+'.md')
$lines=if($Shape -eq 'headings') {
 [string[]](0..4999 | ForEach-Object { '# Heading {0:D5} - navigation test' -f $_ })
} else {
 [string[]](0..299 | ForEach-Object { ('Line {0:D5} ' -f $_) + ('中文 🎉 wrapped text ' * 80) })
}
[IO.File]::WriteAllLines($document, [string[]]$lines, [Text.UTF8Encoding]::new($false))
$ini=Join-Path (Split-Path $Exe) 'FeatherMD.ini'
if(Test-Path -LiteralPath $ini){throw 'Refusing to overwrite existing portable settings'}
Set-Content -LiteralPath $ini -Value "theme=light
font_size=18
language=zh-CN" -Encoding utf8
$oldSnapshot=$env:FEATHERMD_TEST_SNAPSHOT
$env:FEATHERMD_TEST_SNAPSHOT=Join-Path $Output 'viewport.bmp'
$process=$null
$window=[IntPtr]::Zero
function Query([int]$id) { [NavigationSmoke]::SendMessageW($window,0x802a,[IntPtr]$id,[IntPtr]::Zero).ToInt64() }
# The source offset plus wrapped-row ordinal distinguishes progress inside one line.
function Cursor { ((Query 9) * 1MB) + (Query 14) }
function LastCursor { ((Query 15) * 1MB) + (Query 17) }
function Idle {
 for($i=0;$i -lt 500;$i++) { if((Query 11) -eq 0 -and (Query 2) -gt 0){return}; Start-Sleep -Milliseconds 10 }
 throw 'Page load timed out'
}
function Key([int]$key) {
 [void][NavigationSmoke]::SendMessageW($window,0x100,[IntPtr]$key,[IntPtr]::Zero)
 Idle
}
function Capture {
 if((Query 4) -ne 1){throw 'Viewport capture failed'}
}
try {
 $process=Start-Process -FilePath $Exe -ArgumentList ('"'+$document+'"') -WindowStyle Hidden -PassThru
 for($i=0;$i -lt 100;$i++){Start-Sleep -Milliseconds 30;$window=[NavigationSmoke]::Find($process.Id);if($window -ne [IntPtr]::Zero){break}}
 if($window -eq [IntPtr]::Zero){throw 'Window not created'}
 Idle
 Key 0x23
 Capture
 if((Query 16) -lt 3){throw "End left a mostly empty viewport: only $(Query 16) painted row(s)"}
 Copy-Item -LiteralPath $env:FEATHERMD_TEST_SNAPSHOT -Destination (Join-Path $Output "eof-$Shape.bmp")
 $bottom=Cursor
 for($i=0;$i -lt 5;$i++){Key 0x28}
 if((Cursor) -ne $bottom){throw 'Down overscrolled past the final full viewport'}
 Key 0x24
 # A hang guard, not a page budget: page counts follow the window's logical size
 # (a 720x547 logical window needs 1111 PageDowns for the headings fixture).
 $maxPages=10000
 for($i=0;$i -lt $maxPages;$i++) {
  Capture
  $before=Cursor; $last=LastCursor
  Key 0x22
  $after=Cursor
  if($after -gt $last){throw "PageDown skipped visible content at step $i : $before -> $after (last visible $last)"}
  if($after -lt $before){throw "PageDown moved backwards at step $i : $before -> $after"}
  if($after -eq $before){break}
 }
 if($i -eq $maxPages){throw "Forward paging failed to reach EOF: byte=$(Query 9) size=$(Query 10) dpi=$(Query 3) client=$(Query 0)x$(Query 1)"}
 $forward=$i
 Capture
 if((Query 15) -ne ((Query 10)-[Text.Encoding]::UTF8.GetByteCount($lines[-1]+[Environment]::NewLine))){throw "EOF not visible after paging: last=$(Query 15) size=$(Query 10)"}
 for($i=0;$i -lt $maxPages;$i++) {
  $before=Cursor
  Key 0x21
  Capture
  $after=Cursor; $last=LastCursor
  if($last -lt $before){throw "PageUp skipped content at step $i : $before -> $after (last visible $last)"}
  if($after -eq 0){break}
  if($after -ge $before){throw 'Backward paging stopped before the beginning'}
 }
 if($i -eq $maxPages){throw "Backward paging failed to reach beginning: byte=$(Query 9) dpi=$(Query 3) client=$(Query 0)x$(Query 1)"}
 $backward=$i+1
 # Every font size reachable by the +/- controls; actual GDI layout, not synthetic rows.
 for($i=0;$i -lt 15;$i++){[void][NavigationSmoke]::SendMessageW($window,0x111,[IntPtr]203,[IntPtr]::Zero)}
 foreach($font in 12..36 | Where-Object {$_ % 2 -eq 0}) {
  if((Query 12) -ne $font){throw 'Zoom setting mismatch'}
  Key 0x23; Capture
  if((Query 16) -lt 3){throw "EOF viewport collapsed at font $font"}
  $bottom=Cursor; Key 0x28
  if((Cursor) -ne $bottom){throw "EOF overscroll at font $font"}
  [void][NavigationSmoke]::SendMessageW($window,0x111,[IntPtr]202,[IntPtr]::Zero)
 }
 $metrics=[ordered]@{result='PASS';shape=$Shape;forward_pages=$forward;backward_pages=$backward;native_zoom_sizes=13;exe_bytes=(Get-Item -LiteralPath $Exe).Length}
 $metrics|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $Output 'navigation.json') -Encoding utf8
 $metrics|ConvertTo-Json
} finally {
 try {
  if($process -and -not $process.HasExited){[void][NavigationSmoke]::PostMessageW($window,0x10,[IntPtr]::Zero,[IntPtr]::Zero);if(-not $process.WaitForExit(5000)){$process.Kill();throw 'Reader failed to exit cleanly'}}
 } finally {
  Remove-Item -LiteralPath $ini -ErrorAction SilentlyContinue
  $env:FEATHERMD_TEST_SNAPSHOT=$oldSnapshot
 }
}

. "$PSScriptRoot/verify-ui-test-host.ps1" -Output $Output -SecondaryMonitor:$SecondaryMonitor
