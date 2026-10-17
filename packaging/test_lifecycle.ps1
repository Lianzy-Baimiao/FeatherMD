param(
    [Parameter(Mandatory=$true)][string]$Package,
    [Parameter(Mandatory=$true)][string]$Upgrade,
    [Parameter(Mandatory=$true)][string]$SignTool,
    [Parameter(Mandatory=$true)][string]$Output
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
# This script creates temporary trust. It must never run on a user's computer.
if($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'Only run in a disposable GitHub-hosted Windows runner'
}
$Package=(Resolve-Path -LiteralPath $Package).Path
$Upgrade=(Resolve-Path -LiteralPath $Upgrade).Path
$SignTool=(Resolve-Path -LiteralPath $SignTool).Path
$Output=[IO.Path]::GetFullPath($Output)
$prefix=[IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\')+'\'
if(-not $Output.StartsWith($prefix,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $Output)){throw 'Use a new RUNNER_TEMP output directory'}
if(Get-AppxPackage -Name FeatherMD.StoreTest){throw 'Test package already installed'}
New-Item -ItemType Directory -Path $Output | Out-Null
$private=Join-Path $env:RUNNER_TEMP ('feathermd-signing-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $private | Out-Null
Add-Type -AssemblyName System.IO.Compression.FileSystem
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
[ComImport, Guid("43826d1e-e718-42ee-bc55-a1e261c37bfe"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IShellItem {}
[ComImport, Guid("b63ea76d-1f85-456f-a19c-48159efa858b"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IShellItemArray {}
[ComImport, Guid("2e941141-7f97-4756-ba1d-9decde894a3d"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IApplicationActivationManager {
 [PreserveSig] int ActivateApplication([MarshalAs(UnmanagedType.LPWStr)] string id,
  [MarshalAs(UnmanagedType.LPWStr)] string args, uint options, out uint pid);
 [PreserveSig] int ActivateForFile([MarshalAs(UnmanagedType.LPWStr)] string id,
  IShellItemArray items, [MarshalAs(UnmanagedType.LPWStr)] string verb, out uint pid);
}
[ComImport, Guid("45BA127D-10A8-46EA-8AB7-56EA9078943C")]
class ApplicationActivationManager {}
public static class PackageActivation {
 [DllImport("shell32.dll", CharSet=CharSet.Unicode, PreserveSig=false)]
 static extern void SHCreateItemFromParsingName(string path, IntPtr context, ref Guid iid, out IShellItem item);
 [DllImport("shell32.dll", PreserveSig=false)]
 static extern void SHCreateShellItemArrayFromShellItem(IShellItem item, ref Guid iid, out IShellItemArray array);
 public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr data);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr hwnd, StringBuilder s, int count);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowTextW(IntPtr hwnd, StringBuilder s, int count);
 [DllImport("user32.dll")] static extern IntPtr SendMessageTimeoutW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp, uint flags, uint timeout, out IntPtr result);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 public static long Query(IntPtr hwnd, int query) {
  IntPtr result;
  if(SendMessageTimeoutW(hwnd,0x802a,(IntPtr)query,IntPtr.Zero,2,5000,out result)==IntPtr.Zero) throw new Exception("Packaged reader did not respond");
  return result.ToInt64();
 }
 public static string Title(IntPtr hwnd){var s=new StringBuilder(1024);GetWindowTextW(hwnd,s,s.Capacity);return s.ToString();}
 public static IntPtr Find(uint pid){IntPtr found=IntPtr.Zero;
  EnumWindows((h,d)=>{uint p;GetWindowThreadProcessId(h,out p);var s=new StringBuilder(128);GetClassNameW(h,s,s.Capacity);
   if(p==pid && s.ToString()=="FeatherMD.Native.Reader"){found=h;return false;}return true;},IntPtr.Zero);return found;
 }
 public static uint Open(string id,string path){
  IShellItem item=null;IShellItemArray array=null;
  var manager=(IApplicationActivationManager)new ApplicationActivationManager();
  try {var itemId=typeof(IShellItem).GUID;SHCreateItemFromParsingName(path,IntPtr.Zero,ref itemId,out item);
   var arrayId=typeof(IShellItemArray).GUID;SHCreateShellItemArrayFromShellItem(item,ref arrayId,out array);
   uint pid;Marshal.ThrowExceptionForHR(manager.ActivateForFile(id,array,"open",out pid));return pid;
  } finally {if(array!=null)Marshal.ReleaseComObject(array);if(item!=null)Marshal.ReleaseComObject(item);Marshal.ReleaseComObject(manager);}
 }
}
'@
function Identity([string]$file) {
    $zip=[IO.Compression.ZipFile]::OpenRead($file)
    try {
        $entry=$zip.GetEntry('AppxManifest.xml')
        if(-not $entry){throw 'No package manifest'}
        $reader=[IO.StreamReader]::new($entry.Open())
        try{$xml=[xml]$reader.ReadToEnd()}finally{$reader.Dispose()}
        if($xml.Package.Identity.Name -ne 'FeatherMD.StoreTest' -or $xml.Package.Identity.Publisher -ne 'CN=FeatherMD Store Test'){throw 'Only the test identity is allowed'}
        return [version]$xml.Package.Identity.Version
    } finally {$zip.Dispose()}
}
$v1=Identity $Package; $v2=Identity $Upgrade
if($v2 -le $v1){throw 'Upgrade version must increase'}
$cert=$null; $installed=$false; $cleanupErrors=@()
$result=[ordered]@{result='FAIL';stage='sign';installed_tested=$false;shell_activation_tested=$false;wack='not-run';cleanup_errors=@()}
function Sign([string]$inputFile,[string]$name) {
    $copy=Join-Path $private $name
    Copy-Item -LiteralPath $inputFile -Destination $copy
    & $SignTool sign /fd SHA256 /sha1 $cert.Thumbprint /s My $copy | Out-Host
    if($LASTEXITCODE){throw 'Test signing failed'}
    return $copy
}
try {
    $cert=New-SelfSignedCertificate -Type Custom -Subject 'CN=FeatherMD Store Test' -KeyUsage DigitalSignature -KeyExportPolicy NonExportable -CertStoreLocation Cert:\CurrentUser\My -NotAfter (Get-Date).AddDays(2) -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3')
    $cer=Join-Path $private 'test.cer'
    Export-Certificate -Cert $cert -FilePath $cer | Out-Null
    Import-Certificate -FilePath $cer -CertStoreLocation Cert:\LocalMachine\TrustedPeople | Out-Null
    $signed=Sign $Package 'initial.msix'
    $signedUpgrade=Sign $Upgrade 'upgrade.msix'
    $result.stage='install'
    Add-AppxPackage -Path $signed
    $installed=$true
    $app=Get-AppxPackage -Name FeatherMD.StoreTest
    if(-not $app -or [version]$app.Version -ne $v1){throw 'Initial package not installed'}
    $family=$app.PackageFamilyName
    $result.family=$family
    $exe=Join-Path $app.InstallLocation 'FeatherMD.exe'
    $result.stage='shell-file-activation'
    $activated=@()
    foreach($ext in @('.md','.markdown','.mdown','.mkd')) {
        $file=Join-Path $Output (([string][char]0x9605)+([string][char]0x8bfb)+' spaced file'+$ext)
        [IO.File]::WriteAllText($file,"# Shell activation`n`nUnicode and spaces.`n",[Text.UTF8Encoding]::new($false))
        $processId=[PackageActivation]::Open("$family!FeatherMD",$file)
        $process=[Diagnostics.Process]::GetProcessById($processId)
        $window=[IntPtr]::Zero
        try {
            for($i=0;$i -lt 400;$i++) {
                if($process.HasExited){throw 'Shell-activated reader exited early'}
                $window=[PackageActivation]::Find($processId)
                if($window -ne [IntPtr]::Zero -and [PackageActivation]::Query($window,10) -eq (Get-Item -LiteralPath $file).Length){break}
                Start-Sleep -Milliseconds 25
            }
            if($i -eq 400){throw "File activation did not load $ext"}
            if([PackageActivation]::Query($window,97) -ne 1 -or [PackageActivation]::Query($window,98) -ne 1){throw 'Shell activation lost Store/package identity'}
            if(-not [PackageActivation]::Title($window).StartsWith([IO.Path]::GetFileName($file))){throw 'Shell activation lost the Unicode file name'}
            $activated+=$ext
        } finally {
            if($window -ne [IntPtr]::Zero){[void][PackageActivation]::PostMessageW($window,0x10,[IntPtr]::Zero,[IntPtr]::Zero)}
            if(-not $process.WaitForExit(5000)){$process.Kill();throw 'Shell-activated process failed to close'}
            $process.Dispose()
        }
    }
    $result.shell_activation_tested=$true
    $result.activated_extensions=$activated
    $result.stage='gui'
    & pwsh -NoProfile -File "$PSScriptRoot/../scripts/store-smoke.ps1" -Exe $exe -PackageFamily $family -Output (Join-Path $Output 'gui')
    if($LASTEXITCODE){throw 'Installed GUI test failed'}
    $ini=Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) "Packages\$family\LocalState\settings.ini"
    [IO.File]::WriteAllText($ini,"language=en`ntheme=sepia`n",[Text.UTF8Encoding]::new($false))
    $hash=(Get-FileHash -LiteralPath $ini).Hash
    $result.stage='upgrade'
    Add-AppxPackage -Path $signedUpgrade
    $updated=Get-AppxPackage -Name FeatherMD.StoreTest
    if([version]$updated.Version -ne $v2 -or $updated.PackageFamilyName -ne $family){throw 'Upgrade identity/version mismatch'}
    if((Get-FileHash -LiteralPath $ini).Hash -ne $hash){throw 'Upgrade lost settings'}
    $result.stage='uninstall'
    Remove-AppxPackage -Package $updated.PackageFullName
    $installed=$false
    if(Get-AppxPackage -Name FeatherMD.StoreTest){throw 'Package remains after uninstall'}
    $result.installed_tested=$true
    $result.result='PASS'
    $result.stage='complete'
} catch {
    $result.error=$_.Exception.Message
    throw
} finally {
    if($installed){try{Get-AppxPackage -Name FeatherMD.StoreTest | Remove-AppxPackage}catch{$cleanupErrors+=$_.Exception.Message}}
    if($cert){
        foreach($store in @('Cert:\LocalMachine\TrustedPeople','Cert:\CurrentUser\My')){
            try{Remove-Item -LiteralPath "$store\$($cert.Thumbprint)" -ErrorAction Stop}catch{$cleanupErrors+=$_.Exception.Message}
        }
    }
    $result.cleanup_errors=$cleanupErrors
    if($cleanupErrors.Count){$result.result='FAIL'}
    $result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $Output 'lifecycle.json') -Encoding utf8
}
if($cleanupErrors.Count){throw 'Cleanup did not finish'}
