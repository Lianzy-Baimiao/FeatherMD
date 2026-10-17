param(
    [string]$Exe = "$PSScriptRoot\..\target\store\release\feathermd.exe",
    [string]$Output = "$PSScriptRoot\..\test-artifacts\store",
    [string]$PackageFamily = "",
    [switch]$AllowDesktopUi,
    [switch]$SecondaryMonitor
)
# Native-message tests; no Store launch, registration, certificate or defaults changes.
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/require-ui-test-host.ps1" -AllowDesktopUi:$AllowDesktopUi -SecondaryMonitor:$SecondaryMonitor -Exe $Exe
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class StoreSmoke {
 public delegate bool EnumProc(IntPtr h, IntPtr d);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr d);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr h, StringBuilder s, int count);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int command);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
 [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageW")] static extern IntPtr ReadText(IntPtr h,uint m,IntPtr w,StringBuilder s);
 public static string Text(IntPtr h) {var s=new StringBuilder(4096); ReadText(h,13,(IntPtr)s.Capacity,s); return s.ToString();}
 public static IntPtr Find(uint pid,string cls,string title) {
  IntPtr found=IntPtr.Zero;
  EnumWindows((h,d)=>{uint p;GetWindowThreadProcessId(h,out p);var c=new StringBuilder(128);GetClassNameW(h,c,c.Capacity);
   if(p==pid && c.ToString()==cls && (String.IsNullOrEmpty(title) || Text(h)==title)){found=h;return false;}return true;},IntPtr.Zero);
  return found;
 }
}
'@
$Exe = (Resolve-Path -LiteralPath $Exe).Path
$Output = [IO.Path]::GetFullPath($Output)
if (Test-Path -LiteralPath $Output) { throw 'Use a new Store smoke output folder' }
New-Item -ItemType Directory -Path $Output | Out-Null
$snapshot = Join-Path $Output 'store.bmp'
$local = [Environment]::GetFolderPath('LocalApplicationData')
if ($PackageFamily) {
    if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $PackageFamily -notmatch '^FeatherMD\.StoreTest_[a-z0-9]+$') {
        throw 'Packaged smoke only supports the test identity on a disposable hosted runner'
    }
    $ini = Join-Path $local "Packages\$PackageFamily\LocalState\settings.ini"
} else {
    $ini = Join-Path $local 'FeatherMD\StorePreview\settings.ini'
}
if (Test-Path -LiteralPath $ini) { throw 'Refusing to overwrite existing Store/preview settings' }
$adjacent = Join-Path (Split-Path $Exe) 'FeatherMD.ini'
if (-not $PackageFamily -and (Test-Path -LiteralPath $adjacent)) { throw 'Adjacent INI already exists' }
$oldSnapshot=$env:FEATHERMD_TEST_SNAPSHOT
$oldDpi=[StoreSmoke]::SetThreadDpiAwarenessContext([IntPtr](-4))
$process=$null; $window=[IntPtr]::Zero; $dialog=[IntPtr]::Zero; $notice=[IntPtr]::Zero
$createdIni=$false; $createdAdjacent=$false
$utf8=[Text.UTF8Encoding]::new($false)
$results=@()
function Send([IntPtr]$h,[uint32]$m,[int]$w) { [StoreSmoke]::SendMessageW($h,$m,[IntPtr]$w,[IntPtr]::Zero).ToInt64() }
function Post([IntPtr]$h,[uint32]$m,[int]$w) { if(-not [StoreSmoke]::PostMessageW($h,$m,[IntPtr]$w,[IntPtr]::Zero)){throw 'Post failed'} }
function Query([int]$id) { Send $window 0x802a $id }
function WaitWindow([string]$cls,[string]$title) {
    for($i=0;$i -lt 400;$i++) {
        if($process.HasExited){throw 'Reader exited unexpectedly'}
        $h=[StoreSmoke]::Find($process.Id,$cls,$title)
        if($h -ne [IntPtr]::Zero){return $h}
        Start-Sleep -Milliseconds 25
    }
    throw "Window not found: $cls $title"
}
function Hide([IntPtr]$h) {
    for($i=0;$i -lt 8;$i++) { [void][StoreSmoke]::ShowWindow($h,0); Start-Sleep -Milliseconds 30 }
}
function CloseDialog([IntPtr]$h) {
    Post $h 0x10 0
    for($i=0;$i -lt 200 -and [StoreSmoke]::IsWindow($h);$i++){Start-Sleep -Milliseconds 20}
    if([StoreSmoke]::IsWindow($h)){throw 'Dialog did not close'}
}
function Capture([IntPtr]$h,[string]$name) {
    Hide $h
    if((Send $h 0x802a 4) -ne 1){throw "Capture failed: $name"}
    $bitmap=[Drawing.Bitmap]::new($snapshot)
    try{$bitmap.Save((Join-Path $Output "$name.png"),[Drawing.Imaging.ImageFormat]::Png)}finally{$bitmap.Dispose()}
    $query=if($h -eq $window){78}else{5}
    if((Send $h 0x802a $query) -ne 0){throw "Clipped text in $name : $([IO.File]::ReadAllText((Join-Path $Output 'labels.txt')))"}
}
try {
    New-Item -ItemType Directory -Path (Split-Path $ini) -Force | Out-Null
    if(-not $PackageFamily) {
        [IO.File]::WriteAllText($adjacent,"theme=dark`nlanguage=ja`n",$utf8)
        $createdAdjacent=$true
    }
    $document=Join-Path $Output (([string][char]0x9605)+([string][char]0x8bfb)+' spaced document.md')
    [IO.File]::WriteAllText($document,"# Store activation`n`nA local Markdown document.`n",$utf8)
    foreach($language in @('zh-CN','zh-TW','en','ja')) {
        foreach($theme in @('light','dark')) {
            [IO.File]::WriteAllText($ini,"language=$language`ntheme=$theme`nupdate_check=true`nupdate_latest=v99.0.0`n",$utf8)
            $createdIni=$true
            $env:FEATHERMD_TEST_SNAPSHOT=$snapshot
            $process=Start-Process -FilePath $Exe -ArgumentList ('"'+$document+'"') -WindowStyle Hidden -PassThru
            $window=WaitWindow 'FeatherMD.Native.Reader' $null
            for($i=0;$i -lt 500;$i++) {if((Query 11) -eq 0 -and (Query 2) -gt 0){break};Start-Sleep -Milliseconds 20}
            if((Query 2) -le 0){throw 'Document did not load'}
            if((Query 97) -ne 1){throw 'Not a Store build'}
            $packaged=if($PackageFamily){1}else{0}
            if((Query 98) -ne $packaged){throw 'Unexpected package identity'}
            if((Query 99) -ne 1){throw 'No distribution diagnostics'}
            $paths=[IO.File]::ReadAllLines((Join-Path $Output 'distribution.txt'))
            if($paths[0] -ne $ini -or $paths[1] -ne $document){throw "Wrong settings/document path: $paths"}
            if((Query 77) -ne [array]::IndexOf(@('zh-CN','zh-TW','en','ja'),$language)){throw 'Adjacent INI took precedence'}
            if((Query 39) -ne $(if($theme -eq 'light'){1}else{2})){throw 'Theme did not load'}
            Capture $window "$language-$theme-reader"
            Post $window 0x111 417
            $title=if($language -eq 'en'){'Settings'}elseif($language -eq 'zh-CN'){[string][char]0x8bbe+[char]0x7f6e}else{[string][char]0x8a2d+[char]0x5b9a}
            $dialog=WaitWindow '#32770' $title
            for($i=0;$i -lt 400 -and [StoreSmoke]::GetDlgItem($dialog,1166) -eq [IntPtr]::Zero;$i++){Start-Sleep -Milliseconds 20}
            if([StoreSmoke]::GetDlgItem($dialog,1160) -ne [IntPtr]::Zero){throw 'Portable auto-update checkbox is present'}
            if([StoreSmoke]::GetDlgItem($dialog,1162) -ne [IntPtr]::Zero){throw 'Portable download button is present'}
            $button=[StoreSmoke]::Text([StoreSmoke]::GetDlgItem($dialog,1161))
            if($button -notmatch 'Microsoft Store'){throw 'Store update entry is missing'}
            Capture $dialog "$language-$theme-updates"
            # Read the offline notice, without activating a browser or Store.
            Post $dialog 0x111 1166
            $privacy=@{'zh-CN'=([string][char]0x9690+[char]0x79c1+[char]0x8bf4+[char]0x660e+[char]0x2026);'zh-TW'=([string][char]0x96b1+[char]0x79c1+[char]0x8aaa+[char]0x660e+[char]0x2026);en='Privacy notice...';ja=([string][char]0x30d7+[char]0x30e9+[char]0x30a4+[char]0x30d0+[char]0x30b7+[char]0x30fc+[char]0x2026)}
            $privacy.en='Privacy notice'+[char]0x2026
            $notice=WaitWindow '#32770' $privacy[$language]
            Hide $notice
            CloseDialog $notice; $notice=[IntPtr]::Zero
            [void](Send $dialog 0x111 1)
            $dialog=[IntPtr]::Zero
            # Exceed the automatic-check delay even if the machine was very fast.
            Start-Sleep -Milliseconds 3200
            if((Query 51) -ne 0 -or (Query 52) -ne 0 -or (Query 53) -ne 0){throw 'Store edition invoked portable update logic'}
            Post $window 0x111 301
            Post $window 0x111 302
            [void](Query 0)
            Post $window 0x111 401
            $aboutTitle=@{'zh-CN'=([string][char]0x5173+[char]0x4e8e+' FeatherMD');'zh-TW'=([string][char]0x95dc+[char]0x65bc+' FeatherMD');en='About FeatherMD';ja=('FeatherMD '+[char]0x306b+[char]0x3064+[char]0x3044+[char]0x3066)}
            $dialog=WaitWindow '#32770' $aboutTitle[$language]
            Capture $dialog "$language-$theme-about"
            CloseDialog $dialog; $dialog=[IntPtr]::Zero
            Post $window 0x10 0
            if(-not $process.WaitForExit(5000)){throw 'Reader failed to exit'}
            if([IO.File]::ReadAllText($ini) -notmatch 'update_check=false'){throw 'Saved Store settings retained auto-update'}
            $results+=@{language=$language;theme=$theme;store=$true;packaged=[bool]$PackageFamily;github_checks=0;settings_isolated=$true;unicode_path=$true}
        }
    }
    @{result='PASS';cases=$results;limitations='Native-message tests; no Store launch, default-app changes or Shell file activation.'} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Output 'store.json') -Encoding utf8
    Write-Output "PASS: $($results.Count) Store cases; updates, About, settings, Unicode paths, privacy."
} finally {
    foreach($h in @($notice,$dialog)){if($h -ne [IntPtr]::Zero -and [StoreSmoke]::IsWindow($h)){Post $h 0x10 0;Start-Sleep -Milliseconds 100}}
    if($process -and -not $process.HasExited){Post $window 0x10 0;if(-not $process.WaitForExit(5000)){$process.Kill()}}
    if($createdIni){Remove-Item -LiteralPath $ini}
    if($createdAdjacent){Remove-Item -LiteralPath $adjacent}
    $env:FEATHERMD_TEST_SNAPSHOT=$oldSnapshot
    [void][StoreSmoke]::SetThreadDpiAwarenessContext($oldDpi)
}

. "$PSScriptRoot/verify-ui-test-host.ps1" -Output $Output -SecondaryMonitor:$SecondaryMonitor
