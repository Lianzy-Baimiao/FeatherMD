param([string]$Exe = "$PSScriptRoot\..\target\release\feathermd.exe", [string]$Document = "$PSScriptRoot\..\docs\demo.md", [string]$Output = "$PSScriptRoot\..\test-artifacts", [switch]$AllowDesktopUi, [switch]$SecondaryMonitor)
$ErrorActionPreference='Stop'
. "$PSScriptRoot/require-ui-test-host.ps1" -AllowDesktopUi:$AllowDesktopUi -SecondaryMonitor:$SecondaryMonitor -Exe $Exe
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class NativeSmoke {
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
$Exe=(Resolve-Path -LiteralPath $Exe).Path
$Document=(Resolve-Path -LiteralPath $Document).Path
# Portable test settings keep the user's AppData settings untouched.
$ini=Join-Path (Split-Path $Exe) 'FeatherMD.ini'
if(Test-Path -LiteralPath $ini){throw 'Refusing to overwrite existing portable settings'}
Set-Content -LiteralPath $ini -Value "theme=light`nfont_size=18`nlanguage=zh-CN" -Encoding utf8
$env:FEATHERMD_TEST_SNAPSHOT=Join-Path (Resolve-Path -LiteralPath $Output).Path "snapshot.bmp"
$process=$null
try {
 $clock=[Diagnostics.Stopwatch]::StartNew()
 $process=Start-Process -FilePath $Exe -ArgumentList ('"'+$Document+'"') -WindowStyle Hidden -PassThru
 $window=[IntPtr]::Zero
 for($i=0;$i -lt 100;$i++){Start-Sleep -Milliseconds 30;$window=[NativeSmoke]::Find($process.Id);if($window -ne [IntPtr]::Zero){break}}
 if($window -eq [IntPtr]::Zero){throw 'Native window not created'}
 $windowMs=$clock.Elapsed.TotalMilliseconds
 Write-Output ('HWND='+$window+' WIDTH='+[NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]0,[IntPtr]::Zero)+' HEIGHT='+[NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]1,[IntPtr]::Zero))
 for($i=0;$i -lt 500;$i++) { if([NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]2,[IntPtr]::Zero).ToInt64() -gt 0){break};Start-Sleep -Milliseconds 10 }
 if($i -eq 500){throw 'First page layout timed out'}
 $readyMs=$clock.Elapsed.TotalMilliseconds
 # Keep first-layout timing independent of the optional outline worker.
 for($i=0;$i -lt 500;$i++){if([NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]33,[IntPtr]::Zero).ToInt64() -eq 0){break};Start-Sleep -Milliseconds 10}
 if($i -eq 500){throw 'Outline did not finish within the bounded scan timeout'}
 Write-Output ('ROWS='+[NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]2,[IntPtr]::Zero)+' DPI='+[NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]3,[IntPtr]::Zero))
 function Capture([string]$name){
  if([NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]4,[IntPtr]::Zero) -ne [IntPtr]1){throw 'Internal GDI capture failed'}
  $bmp=[Drawing.Bitmap]::new($env:FEATHERMD_TEST_SNAPSHOT)
  $bmp.Save((Join-Path $Output ($name+'.png')),[Drawing.Imaging.ImageFormat]::Png)
  # The outline is at the left and Find's results at the right: sample the gutter
  # just left of the reading column.
  $gutter=[Math]::Max(0,[NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]80,[IntPtr]::Zero).ToInt64()-[Math]::Floor(6*[NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]3,[IntPtr]::Zero).ToInt64()/96))
  $a=$bmp.GetPixel(5,5).ToArgb();$b=$bmp.GetPixel([int]$gutter,[Math]::Min(150,$bmp.Height-1)).ToArgb();$bmp.Dispose()
  if($a -eq $b){throw 'Header and document background did not render'}
 }
 Capture 'reading-light'
 $paintMs=$clock.Elapsed.TotalMilliseconds
 [void][NativeSmoke]::SendMessageW($window,0x111,[IntPtr]411,[IntPtr]::Zero)
 Capture 'reading-outline-hidden'
 [void][NativeSmoke]::SendMessageW($window,0x111,[IntPtr]411,[IntPtr]::Zero)
 [void][NativeSmoke]::SendMessageW($window,0x111,[IntPtr]201,[IntPtr]::Zero)
 Capture 'reading-dark'
 [void][NativeSmoke]::SendMessageW($window,0x111,[IntPtr]102,[IntPtr]::Zero)
 $edit=[NativeSmoke]::FindWindowExW($window,[IntPtr]::Zero,'EDIT',$null)
 if($edit -eq [IntPtr]::Zero){throw 'Search edit not found'}
 [void][NativeSmoke]::SendText($edit,0x0c,[IntPtr]::Zero,'Rust')
 [void][NativeSmoke]::PostMessageW($edit,0x100,[IntPtr]13,[IntPtr]::Zero)
 Start-Sleep -Milliseconds 200
 Write-Output ('SEARCH found='+[NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]5,[IntPtr]::Zero)+' query='+[NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]6,[IntPtr]::Zero)+' edit='+$edit+' nativeedit='+[NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]7,[IntPtr]::Zero)+' ticket='+[NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]8,[IntPtr]::Zero))
 if([NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]5,[IntPtr]::Zero).ToInt64() -lt 0){throw 'Search failed to find Rust'}
 Capture 'search'
 [void][NativeSmoke]::PostMessageW($edit,0x100,[IntPtr]27,[IntPtr]::Zero)
 Start-Sleep -Milliseconds 50
 # Navigate to the middle through the actual edit + Enter path.
 [void][NativeSmoke]::SendMessageW($window,0x111,[IntPtr]103,[IntPtr]::Zero)
 [void][NativeSmoke]::SendText($edit,0x0c,[IntPtr]::Zero,'50')
 [void][NativeSmoke]::PostMessageW($edit,0x100,[IntPtr]13,[IntPtr]::Zero)
 Start-Sleep -Milliseconds 50
 for($i=0;$i -lt 500;$i++){if([NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]11,[IntPtr]::Zero).ToInt64() -eq 0){break};Start-Sleep -Milliseconds 10}
 $size=[NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]10,[IntPtr]::Zero).ToInt64()
 $offset=[NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]9,[IntPtr]::Zero).ToInt64()
 if($size -gt 1MB -and [Math]::Abs($offset/$size-0.5) -gt 0.01){throw "Mid-file navigation failed: $offset / $size"}
 Capture 'middle'
 # Sampling another process during an invalidated paint can count temporary DC/pen objects.
 # The minimum over a short quiescence window still catches retained leaks without
 # mistaking an in-flight paint (e.g. 30 -> 43 -> 30 handles) for permanent growth.
 function StableGdi {
  $minimum = [int]::MaxValue
  for ($sample=0; $sample -lt 12; $sample++) {
   [void][NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]0,[IntPtr]::Zero)
   $minimum = [Math]::Min($minimum,[NativeSmoke]::GetGuiResources($process.Handle,0))
   Start-Sleep -Milliseconds 5
  }
  return $minimum
 }
 # Warm the picker fonts, then include repeated real overlay paints in the leak check.
 [void][NativeSmoke]::SendMessageW($window,0x111,[IntPtr]412,[IntPtr]::Zero)
 Capture 'theme-picker'
 [void][NativeSmoke]::SendMessageW($window,0x100,[IntPtr]27,[IntPtr]::Zero)
 $gdiBefore=StableGdi
 for($i=0;$i -lt 50;$i++){
  [void][NativeSmoke]::SendMessageW($window,0x111,[IntPtr]205,[IntPtr]::Zero)
  [void][NativeSmoke]::SendMessageW($window,0x111,[IntPtr]201,[IntPtr]::Zero)
  if ($i -lt 10) {
   [void][NativeSmoke]::SendMessageW($window,0x111,[IntPtr]412,[IntPtr]::Zero)
   if ([NativeSmoke]::SendMessageW($window,0x802a,[IntPtr]4,[IntPtr]::Zero) -ne [IntPtr]1) { throw 'Theme picker stress paint failed' }
   [void][NativeSmoke]::SendMessageW($window,0x100,[IntPtr]27,[IntPtr]::Zero)
  }
 }
 $gdiAfter=StableGdi
 if($gdiAfter -gt $gdiBefore+10){throw "GDI handle growth: $gdiBefore -> $gdiAfter"}
 $process.Refresh()
 $metrics=[ordered]@{window_created_ms=[Math]::Round($windowMs,2);first_layout_ms=[Math]::Round($readyMs,2);first_capture_ms=[Math]::Round($paintMs,2);working_set_mib=[Math]::Round($process.WorkingSet64/1MB,2);private_mib=[Math]::Round($process.PrivateMemorySize64/1MB,2);gdi_before=$gdiBefore;gdi_after=$gdiAfter;exe_bytes=(Get-Item -LiteralPath $Exe).Length;document=$Document;result='PASS'}
 $metrics|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $Output 'ui-smoke.json') -Encoding utf8
 $metrics|ConvertTo-Json
} finally {
 if($process -and -not $process.HasExited){[void][NativeSmoke]::PostMessageW($window,0x10,[IntPtr]::Zero,[IntPtr]::Zero);if(-not $process.WaitForExit(5000)){$process.Kill();throw "Reader failed to exit cleanly"}}
 Remove-Item -LiteralPath $ini -ErrorAction SilentlyContinue
 Remove-Item Env:\FEATHERMD_TEST_SNAPSHOT -ErrorAction SilentlyContinue
}

. "$PSScriptRoot/verify-ui-test-host.ps1" -Output $Output -SecondaryMonitor:$SecondaryMonitor
