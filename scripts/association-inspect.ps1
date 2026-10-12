param([string]$Extension = '.md', [switch]$RequireFeatherMD)
# Read-only probe: query the same Shell association handlers used by Open With.
$ErrorActionPreference = 'Stop'
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
[ComImport, Guid("F04061AC-1659-4A3F-A954-775AA57FC083"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IAssocHandler {
 [PreserveSig] int GetName([MarshalAs(UnmanagedType.LPWStr)] out string name);
 [PreserveSig] int GetUIName([MarshalAs(UnmanagedType.LPWStr)] out string name);
}
[ComImport, Guid("973810AE-9599-4B88-9E4D-6EE98C9552DA"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IEnumAssocHandlers {
 [PreserveSig] int Next(uint count, [MarshalAs(UnmanagedType.Interface)] out IAssocHandler handler, out uint fetched);
}
public static class AssociationInspect {
 [DllImport("shell32.dll", CharSet=CharSet.Unicode, PreserveSig=true)]
 static extern int SHAssocEnumHandlers(string extension, uint filter, out IEnumAssocHandlers handlers);
 public static string[] Names(string extension) {
  IEnumAssocHandlers handlers;
  Marshal.ThrowExceptionForHR(SHAssocEnumHandlers(extension, 0, out handlers));
  var result = new List<string>();
  try {
   IAssocHandler handler; uint fetched;
   while (true) {
    int hr = handlers.Next(1, out handler, out fetched);
    Marshal.ThrowExceptionForHR(hr);
    if (fetched == 0) break;
    try {
     string name, label;
     Marshal.ThrowExceptionForHR(handler.GetName(out name));
     Marshal.ThrowExceptionForHR(handler.GetUIName(out label));
     result.Add(label + " | " + name);
    } finally { Marshal.ReleaseComObject(handler); }
   }
  } finally { Marshal.ReleaseComObject(handlers); }
  return result.ToArray();
 }
}
"@
$handlers = [AssociationInspect]::Names($Extension)
$handlers | Write-Output
if ($RequireFeatherMD -and -not ($handlers -match 'FeatherMD')) {
 throw "FAIL: Windows Shell does not offer FeatherMD for $Extension"
}
if ($RequireFeatherMD) { Write-Output "PASS: Windows Shell offers FeatherMD for $Extension (not proof of default selection or Settings UI)" }
