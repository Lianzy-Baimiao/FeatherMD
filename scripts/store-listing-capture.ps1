param(
    [Parameter(Mandatory=$true)][string]$Exe,
    [Parameter(Mandatory=$true)][string]$Output
)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'Listing captures run only on a disposable GitHub-hosted Windows runner.'
}
$Exe = (Resolve-Path -LiteralPath $Exe).Path
$expected = 'c41056040d245f4e2a2afde62ff954514d48c01b92803684c65d22f0ba594c10'
if ((Get-FileHash -LiteralPath $Exe -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) {
    throw 'Only the frozen, verified Store candidate may be captured.'
}
$Output = [IO.Path]::GetFullPath($Output)
if (Test-Path -LiteralPath $Output) { throw 'Use a new capture directory.' }
New-Item -ItemType Directory -Path $Output | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class ListingCapture {
 public delegate bool EnumProc(IntPtr h, IntPtr d);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc p, IntPtr d);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr h,StringBuilder s,int n);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int n);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int height,uint flags);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 public static IntPtr Find(uint pid) {
  IntPtr found=IntPtr.Zero;
  EnumWindows((h,d)=>{uint p; GetWindowThreadProcessId(h,out p); var s=new StringBuilder(128); GetClassNameW(h,s,s.Capacity);
   if(p==pid && s.ToString()=="FeatherMD.Native.Reader"){found=h;return false;} return true;},IntPtr.Zero);
  return found;
 }
}
'@
$ini = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'FeatherMD/StorePreview/settings.ini'
if (Test-Path -LiteralPath $ini) { throw 'Refusing to replace existing settings.' }
New-Item -ItemType Directory -Path (Split-Path $ini) -Force | Out-Null
$oldSnapshot = $env:FEATHERMD_TEST_SNAPSHOT
$oldDpi = [ListingCapture]::SetThreadDpiAwarenessContext([IntPtr](-4))
$utf8 = [Text.UTF8Encoding]::new($false)
$createdIni = $false
$process = $null
$window = [IntPtr]::Zero
$records = @()
function Query([int]$id) { [ListingCapture]::SendMessageW($window,0x802a,[IntPtr]$id,[IntPtr]::Zero).ToInt64() }
function CloseReader {
    if ($process -and -not $process.HasExited) {
        [void][ListingCapture]::PostMessageW($window,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
        if (-not $process.WaitForExit(5000)) { $process.Kill(); throw 'Reader failed to close.' }
    }
}
try {
    foreach ($language in @('zh-CN','zh-TW','en','ja')) {
        foreach ($view in @('light','dark','source','tables')) {
            $theme = if ($view -eq 'dark') { 'dark' } else { 'light' }
            $raw = if ($view -eq 'source') { 'true' } else { 'false' }
            $sample = if ($view -eq 'tables') { 'tables' } else { 'reading' }
            $document = (Resolve-Path -LiteralPath "$PSScriptRoot/../docs/store-samples/$language-$sample.md").Path
            [IO.File]::WriteAllText($ini,"language=$language`ntheme=$theme`nraw=$raw`nfont_size=20`nwide=false`noutline=true`nimages=false`nupdate_check=false`n",$utf8)
            $createdIni = $true
            $snapshot = Join-Path $Output "$language-$view.bmp"
            $env:FEATHERMD_TEST_SNAPSHOT = $snapshot
            $process = Start-Process -FilePath $Exe -ArgumentList ('"'+$document+'"') -WindowStyle Hidden -PassThru
            $window = [IntPtr]::Zero
            for ($i=0; $i -lt 400; $i++) {
                if ($process.HasExited) { throw 'Reader exited before capture.' }
                $window = [ListingCapture]::Find($process.Id)
                if ($window -ne [IntPtr]::Zero -and (Query 11) -eq 0 -and (Query 2) -gt 0) { break }
                Start-Sleep -Milliseconds 25
            }
            if ($window -eq [IntPtr]::Zero -or (Query 2) -le 0 -or (Query 97) -ne 1) { throw 'Store reader was not ready.' }
            [void][ListingCapture]::ShowWindow($window,0)
            if (-not [ListingCapture]::SetWindowPos($window,[IntPtr]::Zero,0,0,1600,1000,0x14)) { throw 'Resize failed.' }
            # Render the actual hidden window at listing resolution, without resampling.
            for ($i=0; $i -lt 200; $i++) {
                Start-Sleep -Milliseconds 25
                if ((Query 11) -eq 0) { break }
            }
            if ((Query 4) -ne 1) { throw 'Native snapshot failed.' }
            if ((Query 78) -ne 0) { throw 'Reader label clipping detected.' }
            $png = Join-Path $Output "$language-$view.png"
            $bitmap = [Drawing.Bitmap]::new($snapshot)
            try {
                if ($bitmap.Width -ne 1600 -or $bitmap.Height -ne 1000) { throw "Unexpected snapshot size: $($bitmap.Width)x$($bitmap.Height)" }
                $bitmap.Save($png,[Drawing.Imaging.ImageFormat]::Png)
            } finally { $bitmap.Dispose() }
            $records += @{language=$language;view=$view;file=[IO.Path]::GetFileName($png);sha256=(Get-FileHash -LiteralPath $png -Algorithm SHA256).Hash.ToLowerInvariant();width=1600;height=1000;sample_sha256=(Get-FileHash -LiteralPath $document -Algorithm SHA256).Hash.ToLowerInvariant()}
            CloseReader
        }
    }
    @{binary_sha256=$expected;source_run=39045802702;capture_run=$env:GITHUB_RUN_ID;capture_commit=$env:GITHUB_SHA;method='Actual native in-process GDI snapshot; extracted Store binary, not installed; no resampling or compositing.';screenshots=$records} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Output 'provenance.json') -Encoding utf8
} finally {
    CloseReader
    if ($createdIni) { Remove-Item -LiteralPath $ini }
    $env:FEATHERMD_TEST_SNAPSHOT = $oldSnapshot
    [void][ListingCapture]::SetThreadDpiAwarenessContext($oldDpi)
}
