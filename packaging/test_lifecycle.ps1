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
if([Threading.Thread]::CurrentThread.GetApartmentState() -ne 'STA') {
    throw 'Shell handler tests require pwsh -STA.'
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
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.ComTypes;
[ComImport, Guid("43826d1e-e718-42ee-bc55-a1e261c37bfe"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IShellItem {
 [PreserveSig] int BindToHandler(IntPtr context, ref Guid handler, ref Guid iid, out IDataObject data);
}
[ComImport, Guid("F04061AC-1659-4A3F-A954-775AA57FC083"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IAssocHandler {
 [PreserveSig] int GetName([MarshalAs(UnmanagedType.LPWStr)] out string name);
 [PreserveSig] int GetUIName([MarshalAs(UnmanagedType.LPWStr)] out string name);
 [PreserveSig] int GetIconLocation([MarshalAs(UnmanagedType.LPWStr)] out string path, out int index);
 [PreserveSig] int IsRecommended();
 [PreserveSig] int MakeDefault([MarshalAs(UnmanagedType.LPWStr)] string description);
 [PreserveSig] int Invoke(IDataObject data);
}
[ComImport, Guid("973810AE-9599-4B88-9E4D-6EE98C9552DA"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IEnumAssocHandlers {
 [PreserveSig] int Next(uint count, out IAssocHandler handler, out uint fetched);
}
[ComImport, Guid("36db0196-9665-46d1-9ba7-d3709eecf9ed"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IObjectWithAppUserModelID {
 [PreserveSig] int SetAppID([MarshalAs(UnmanagedType.LPWStr)] string id);
 [PreserveSig] int GetAppID([MarshalAs(UnmanagedType.LPWStr)] out string id);
}
public static class PackageActivation {
 [DllImport("shell32.dll", CharSet=CharSet.Unicode, PreserveSig=false)]
 static extern void SHCreateItemFromParsingName(string path, IntPtr context, ref Guid iid, out IShellItem item);
 [DllImport("shell32.dll", CharSet=CharSet.Unicode, PreserveSig=false)]
 static extern void SHAssocEnumHandlers(string extension, uint filter, out IEnumAssocHandlers handlers);
 public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr data);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr hwnd, StringBuilder s, int count);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowTextW(IntPtr hwnd, StringBuilder s, int count);
 [DllImport("user32.dll")] static extern IntPtr SendMessageTimeoutW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp, uint flags, uint timeout, out IntPtr result);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
 [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern int GetSystemMetrics(int index);
 // Visible top-level windows (pid | class | title), as evidence when an activation fails.
 public static string[] Windows(){var found=new List<string>();
  EnumWindows((h,d)=>{if(IsWindowVisible(h)){uint p;GetWindowThreadProcessId(h,out p);var c=new StringBuilder(256);GetClassNameW(h,c,c.Capacity);
   found.Add(p+" | "+c+" | "+Title(h));}return true;},IntPtr.Zero);return found.ToArray();
 }
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
 public static string Open(string id,string exe,string path){
  IShellItem item=null;IDataObject data=null;IEnumAssocHandlers handlers=null;IAssocHandler selected=null;
  var candidates=new List<string>();string selectedName=null;
  try {
   SHAssocEnumHandlers(System.IO.Path.GetExtension(path),0,out handlers);
   while(true){IAssocHandler handler;uint fetched;Marshal.ThrowExceptionForHR(handlers.Next(1,out handler,out fetched));
    if(fetched==0)break;
    try {string name,label,appId=null;Marshal.ThrowExceptionForHR(handler.GetName(out name));
     Marshal.ThrowExceptionForHR(handler.GetUIName(out label));
     var withId=handler as IObjectWithAppUserModelID;
     if(withId!=null && withId.GetAppID(out appId)<0)appId=null;
     string description=label+" | "+name+" | "+appId;candidates.Add(description);
     if(String.Equals(appId,id,StringComparison.Ordinal) || String.Equals(name,id,StringComparison.Ordinal) ||
        String.Equals(name,exe,StringComparison.OrdinalIgnoreCase)){
      if(selected!=null)throw new Exception("Ambiguous packaged Shell handlers: "+String.Join("; ",candidates));
      selected=handler;handler=null;selectedName=description;
     }
    } finally {if(handler!=null)Marshal.ReleaseComObject(handler);}
   }
   if(selected==null)throw new Exception("No exact packaged Shell handler: "+String.Join("; ",candidates));
   var itemId=typeof(IShellItem).GUID;SHCreateItemFromParsingName(path,IntPtr.Zero,ref itemId,out item);
   var bhid=new Guid("b8c0bd9f-ed24-455c-83e6-d5390c4fe8c4");var dataId=typeof(IDataObject).GUID;
   Marshal.ThrowExceptionForHR(item.BindToHandler(IntPtr.Zero,ref bhid,ref dataId,out data));
   Marshal.ThrowExceptionForHR(selected.Invoke(data));return selectedName;
  } finally {if(data!=null)Marshal.ReleaseComObject(data);if(item!=null)Marshal.ReleaseComObject(item);
   if(selected!=null)Marshal.ReleaseComObject(selected);if(handlers!=null)Marshal.ReleaseComObject(handlers);}
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
$result=[ordered]@{result='FAIL';stage='sign';installed_tested=$false;shell_activation_tested=$false
    shell_activation_method='IAssocHandler::Invoke (Open With); where Windows answers with its Select an app picker, a capture and a direct launch are recorded';shell_handlers=@();wack='not-run';cleanup_errors=@()}
function Sign([string]$inputFile,[string]$name) {
    $copy=Join-Path $private $name
    Copy-Item -LiteralPath $inputFile -Destination $copy
    & $SignTool sign /fd SHA256 /sha1 $cert.Thumbprint /s My $copy | Out-Host
    if($LASTEXITCODE){throw 'Test signing failed'}
    return $copy
}
# Each extension's current default (UserChoice ProgId), or 'none'.
function Defaults([string[]]$extensions) {
    $found=[ordered]@{}
    foreach($ext in $extensions){
        $key="Registry::HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\$ext\UserChoice"
        $found[$ext]=try{[string](Get-ItemProperty -LiteralPath $key -ErrorAction Stop).ProgId}catch{'none'}
    }
    return $found
}
# What followed an activation that did not open the reader: new processes (also
# late ones), visible windows, AppModel and application events, the extension's
# registration, a desktop capture, and a direct launch of the same file as a
# control (a process-start check, not an association test).
function Evidence([string]$file,[string]$message,[int[]]$before,[datetime]$since,[int]$format) {
    Start-Sleep -Seconds 3
    $ext=[IO.Path]::GetExtension($file)
    $evidence=[ordered]@{error=$message;direct_ok=$false}
    $evidence.processes=@(Get-Process -ErrorAction SilentlyContinue | ForEach-Object {
        $start=$null; $path=$null
        try{$start=$_.StartTime}catch{}
        try{$path=$_.Path}catch{}
        if($start -and $start -ge $since){"$($_.Id) | $($_.ProcessName) | $path | $($start.ToString('HH:mm:ss.fff'))"}
    })
    $evidence.windows=@([PackageActivation]::Windows())
    $logs=@(@('Application','Microsoft-Windows-AppModel-Runtime/Admin','Microsoft-Windows-TWinUI/Operational','Microsoft-Windows-Shell-Core/Operational','Microsoft-Windows-AppXDeploymentServer/Operational','Microsoft-Windows-AppReadiness/Admin') |
        Where-Object { Get-WinEvent -ListLog $_ -ErrorAction SilentlyContinue })
    $evidence.events=@(try {
        Get-WinEvent -FilterHashtable @{LogName=[string[]]$logs;StartTime=$since} -MaxEvents 80 -ErrorAction Stop | ForEach-Object {
            $text=([string]$_.Message) -replace '\s+',' '
            if($text.Length -gt 400){$text=$text.Substring(0,400)}
            "$($_.TimeCreated.ToString('HH:mm:ss.fff')) | $($_.LogName) | $($_.ProviderName) | $($_.Id) | $text"
        }
    } catch {"events: $($_.Exception.Message)"})
    $evidence.registration=@(foreach($key in @("Registry::HKEY_CURRENT_USER\Software\Classes\$ext\OpenWithProgids","Registry::HKEY_CLASSES_ROOT\$ext\OpenWithProgids","Registry::HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\$ext\UserChoice")) {
        if(Test-Path -LiteralPath $key){
            $item=Get-Item -LiteralPath $key
            "$key = "+(@($item.GetValueNames() | ForEach-Object { $value=$item.GetValue($_); if($value){"$_=$value"}else{$_} }) -join ', ')
        } else {"$key absent"}
    })
    foreach($progid in @(Get-Item -LiteralPath "Registry::HKEY_CLASSES_ROOT\$ext\OpenWithProgids" -ErrorAction SilentlyContinue | ForEach-Object { $_.GetValueNames() } | Where-Object { $_ -like 'AppX*' })) {
        $id=try{(Get-ItemProperty -LiteralPath "Registry::HKEY_CLASSES_ROOT\$progid\Application" -ErrorAction Stop).AppUserModelID}catch{"no Application key ($($_.Exception.Message))"}
        $evidence.registration+="$progid -> $id"
    }
    try {
        Add-Type -AssemblyName System.Drawing
        $bitmap=[Drawing.Bitmap]::new([PackageActivation]::GetSystemMetrics(78),[PackageActivation]::GetSystemMetrics(79))
        $graphics=[Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.CopyFromScreen([PackageActivation]::GetSystemMetrics(76),[PackageActivation]::GetSystemMetrics(77),0,0,$bitmap.Size)
            $bitmap.Save((Join-Path $Output "activation$ext.png"),[Drawing.Imaging.ImageFormat]::Png)
        } finally {$graphics.Dispose();$bitmap.Dispose()}
        $evidence.capture="activation$ext.png"
    } catch {$evidence.capture="capture failed: $($_.Exception.Message)"}
    $readers=@(Get-Process -Name FeatherMD -ErrorAction SilentlyContinue | Where-Object { $_.Id -notin $before })
    $evidence.new_readers=$readers.Count
    $readers | Stop-Process -Force -ErrorAction SilentlyContinue
    $control=$null
    try {
        $control=Start-Process -FilePath $exe -ArgumentList ('"'+$file+'"') -PassThru
        $controlWindow=[IntPtr]::Zero; $length=(Get-Item -LiteralPath $file).Length
        for($i=0;$i -lt 400 -and -not $control.HasExited;$i++) {
            $controlWindow=[PackageActivation]::Find($control.Id)
            if($controlWindow -ne [IntPtr]::Zero -and [PackageActivation]::Query($controlWindow,10) -eq $length){break}
            Start-Sleep -Milliseconds 25
        }
        $evidence.direct_launch=if($control.HasExited){"exited $($control.ExitCode)"}elseif($controlWindow -eq [IntPtr]::Zero){'no window'}else{
            "loaded=$([PackageActivation]::Query($controlWindow,10) -eq $length) store=$([PackageActivation]::Query($controlWindow,97)) packaged=$([PackageActivation]::Query($controlWindow,98)) format=$([PackageActivation]::Query($controlWindow,100))"}
        $evidence.direct_ok=$controlWindow -ne [IntPtr]::Zero -and -not $control.HasExited -and [PackageActivation]::Query($controlWindow,10) -eq $length -and
            [PackageActivation]::Query($controlWindow,97) -eq 1 -and [PackageActivation]::Query($controlWindow,98) -eq 1 -and [PackageActivation]::Query($controlWindow,100) -eq $format
        if($controlWindow -ne [IntPtr]::Zero){[void][PackageActivation]::PostMessageW($controlWindow,0x10,[IntPtr]::Zero,[IntPtr]::Zero)}
        if(-not $control.HasExited){[void]$control.WaitForExit(5000)}
    } catch {$evidence.direct_launch="failed: $($_.Exception.Message)"}
    finally {if($control){try{if(-not $control.HasExited){$control.Kill()}}catch{};$control.Dispose()}}
    return $evidence
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
    $activated=@(); $failures=[ordered]@{}; $pickers=[ordered]@{}
    # Both declared groups, and the format each opens as (0 Markdown, 1 text, 2 log).
    # Interleaved, and every extension is tried, so a failure shows whether it
    # follows the extension, its group or the number of activations.
    $formats=[ordered]@{'.md'=0;'.txt'=1;'.markdown'=0;'.log'=2;'.mdown'=0;'.text'=1;'.mkd'=0}
    $groups=[ordered]@{'feathermd.markdown'=@('.md','.markdown','.mdown','.mkd');'feathermd.plaintext'=@('.txt','.text','.log')}
    $defaults=Defaults @($formats.Keys)
    $result.defaults=$defaults
    foreach($ext in $formats.Keys) {
        $file=Join-Path $Output (([string][char]0x9605)+([string][char]0x8bfb)+' spaced file'+$ext)
        [IO.File]::WriteAllText($file,"# Shell activation`n`nUnicode and spaces.`n",[Text.UTF8Encoding]::new($false))
        $before=@(Get-Process -Name FeatherMD -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
        $openWith=@(Get-Process -Name OpenWith -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
        $since=(Get-Date).AddSeconds(-1)
        $process=$null; $window=[IntPtr]::Zero
        try {
            # Desktop FTAs receive command-line parameters, not a UWP Windows.File event.
            $result.shell_handlers += [PackageActivation]::Open("$family!FeatherMD",$exe,$file)
            for($i=0;$i -lt 400;$i++) {
                $created=@(Get-Process -Name FeatherMD -ErrorAction SilentlyContinue | Where-Object { $_.Id -notin $before -and $_.Path -eq $exe })
                if($created.Count -gt 1){throw 'Shell created more than one matching reader'}
                if($created.Count -eq 1){$process=$created[0];break}
                Start-Sleep -Milliseconds 25
            }
            if(-not $process){throw 'Shell did not create the exact installed reader process'}
            $processId=$process.Id
            for($i=0;$i -lt 400;$i++) {
                if($process.HasExited){throw 'Shell-activated reader exited early'}
                $window=[PackageActivation]::Find($processId)
                if($window -ne [IntPtr]::Zero -and [PackageActivation]::Query($window,10) -eq (Get-Item -LiteralPath $file).Length){break}
                Start-Sleep -Milliseconds 25
            }
            if($i -eq 400){throw "File activation did not load $ext"}
            if([PackageActivation]::Query($window,97) -ne 1 -or [PackageActivation]::Query($window,98) -ne 1){throw 'Shell activation lost Store/package identity'}
            if([PackageActivation]::Query($window,100) -ne $formats[$ext]){throw "Shell-activated $ext opened as format $([PackageActivation]::Query($window,100))"}
            if(-not [PackageActivation]::Title($window).StartsWith([IO.Path]::GetFileName($file))){throw 'Shell activation lost the Unicode file name'}
            $activated+=$ext
        } catch {
            $message=$_.Exception.Message
            # Where a type already has a default app (.txt: Notepad), Windows can answer
            # Open With with its own "Select an app" picker (OpenWith.exe) instead of
            # starting the chosen app. The picker is outside UI Automation's reach
            # here, so it is recorded (capture), not clicked, and the same file is
            # opened directly to show the reader handles it with package identity.
            $shown=-not $process -and @(Get-Process -Name OpenWith -ErrorAction SilentlyContinue | Where-Object { $_.Id -notin $openWith }).Count -gt 0
            $evidence=Evidence $file $message $before $since $formats[$ext]
            if($shown -and $evidence.direct_ok){$pickers[$ext]=$evidence}else{$failures[$ext]=$evidence}
        } finally {
            if($process){
                if($window -ne [IntPtr]::Zero){[void][PackageActivation]::PostMessageW($window,0x10,[IntPtr]::Zero,[IntPtr]::Zero)}
                if(-not $process.WaitForExit(5000)){$process.Kill();$failures["$ext close"]='Shell-activated process failed to close'}
                $process.Dispose()
            }
            # A picker left open would answer the next activation.
            Get-Process -Name OpenWith -ErrorAction SilentlyContinue | Where-Object { $_.Id -notin $openWith } | Stop-Process -Force -ErrorAction SilentlyContinue
        }
    }
    $result.activated_extensions=$activated
    $result.picker_extensions=@($pickers.Keys)
    $result.picker_evidence=$pickers
    # Each declared group must still start the reader through the Shell.
    foreach($group in $groups.Keys){
        if(-not @($groups[$group] | Where-Object { $_ -in $activated }).Count){$failures[$group]='No extension of this group opened through the Shell'}
    }
    # Nothing here may change a default.
    $after=Defaults @($formats.Keys)
    $result.default_changes=@(foreach($ext in $formats.Keys){if($defaults[$ext] -ne $after[$ext]){"$ext $($defaults[$ext]) -> $($after[$ext])"}})
    if($result.default_changes.Count){$failures['defaults']=$result.default_changes -join '; '}
    if($failures.Count){
        $result.shell_failures=$failures
        throw "Shell activation failed for $(@($failures.Keys) -join ', '); opened $($activated -join ', ')"
    }
    $result.shell_activation_tested=$true
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
    $result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $Output 'lifecycle.json') -Encoding utf8
}
if($cleanupErrors.Count){throw 'Cleanup did not finish'}
