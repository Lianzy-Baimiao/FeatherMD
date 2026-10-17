# Packaged desktop file activation research

Date: 2026-10-17. Scope: primary Microsoft documentation and read-only inspection
of `packaging/msix/AppxManifest.xml` and `packaging/test_lifecycle.ps1`. No local
installation, native UI test, registry mutation, or product-code change was made
for this research.

## Likely diagnosis

Cloud follow-up: run **39045655691**, source **0851b6b**, passed all four file
extensions with the unchanged package manifest and `IAssocHandler::Invoke`.
The exact handler AUMID was `FeatherMD.StoreTest_zhfp64hsjpefy!FeatherMD`;
the new installed process retained package identity and loaded each Unicode/spaces
document. Installed GUI, settings-preserving upgrade, uninstall and cleanup also
passed. This confirms the original test-path mismatch for this package; it is
not a universal assertion about all desktop applications. The initial hypothesis
and primary-source reasoning are preserved below.

The strongest hypothesis is an **activation-contract mismatch in the lifecycle
test**, not a demonstrated broken file association. Confidence: medium-high;
the unchanged package still needs a real Shell-handler invocation on the cloud
runner before this can be called confirmed.

- `IApplicationActivationManager::ActivateForFile` explicitly activates the
  `Windows.File` contract. Microsoft says it converts its Shell items into
  `StorageItem` objects delivered through `FileActivatedEventArgs`. This is not
  documentation of ordinary desktop command-line launching. [1]
- FeatherMD is a `packagedClassicApp` / `mediumIL` application that reads its
  document argument from the command line. Microsoft's desktop extension guide
  distinguishes UWP activation-event verb IDs from full-trust packaged apps'
  `Parameters`. It also requires quoting parameters that can contain paths. [2]
- The API error alone does **not** prove that the package is absent: it explicitly
  allows a contract-not-supported explanation. Installation and the same-user
  package identity must be checked independently. Nor do these docs prove that
  every possible packaged desktop application can never use `ActivateForFile`;
  do not turn the narrower evidence into that universal claim.

## The current manifest is not missing a mandatory open verb

Microsoft's desktop FTA example declares `uap:Extension` containing
`uap3:FileTypeAssociation` and `uap:SupportedFileTypes` **without**
`SupportedVerbs`. The same guide describes `SupportedVerbs` separately as adding
file context-menu options. The schema makes the supported-verbs choice optional.
Consequently, no primary source found requires adding an explicit `open` verb
just to obtain normal Open With behavior. [2][3][4]

The current `Executable` + `uap10:RuntimeBehavior="packagedClassicApp"` +
`uap10:TrustLevel="mediumIL"` combination is explicitly valid. The Application
schema says extension activation attributes inherit from Application when not
specified, and recommends these attributes for minimum OS build 19041 or newer.
An omitted legacy `EntryPoint="Windows.FullTrustApplication"` is therefore not
an identified defect for FeatherMD's declared minimum version. [5]

Do not change the manifest and activation API together as the first experiment:
that would conceal which assumption failed. If a later isolated experiment needs
an explicit verb, its correct nesting is `uap2:SupportedVerbs` containing
`uap3:Verb Id="open" Parameters="&quot;%1&quot;"`, with `xmlns:uap2` declared.
This is a diagnostic variant, not a currently justified product fix. [2][4]

## Smallest discriminating cloud probe

Use the already-built, unchanged MSIX and the existing disposable runner only:

1. After installation, record current-user package name/full name/family,
   install location, manifest application ID, and exact expected AUMID
   `<family>!FeatherMD`. Preserve the original failed API and HRESULT as evidence.
2. Call `SHAssocEnumHandlers(L".md", ASSOC_FILTER_NONE, ...)`. Record every
   handler's `GetName`, `GetUIName`, and method HRESULTs. Enumeration is the
   documented way to obtain handlers associated with an extension. [6]
3. If supported, query each handler for `IObjectWithAppUserModelID` and read
   `GetAppID`. Microsoft explicitly describes this interface as exposing an
   `IAssocHandler`'s explicit AUMID. Its availability on every system handler is
   **not** guaranteed by the documentation; handle `E_NOINTERFACE`. [7]
4. Require exactly one handler that matches the expected AUMID or the canonical
   installed `FeatherMD.exe` path returned by `GetName`. `GetName` is documented
   as the full executable path. Log its actual result instead of assuming how
   a packaged handler represents itself. Never select by friendly name alone;
   fail with the enumeration evidence if exact matching is unavailable or
   ambiguous. [8]
5. Create one existing Unicode/spaces `.md` fixture as a Shell item, then call
   `IShellItem::BindToHandler` with `BHID_DataObject` and `IID_IDataObject`.
   `BHID_DataObject` explicitly returns the data object for an item. Call the
   selected `IAssocHandler::Invoke` with this **single-item** data object.
   Microsoft documents this as what an Open With menu does to launch the chosen
   application, making it the appropriate end-to-end Shell test here. [9][10]
6. `Invoke` returns an HRESULT, not a PID. Poll for a newly created process at
   the exact installed EXE path, then require its native reader window, expected
   package identity, document byte length, and Unicode file title. Keep the
   existing bounded timeout and cleanup. A successful call without the expected
   file loaded is not a pass. Repeat for all four extensions only after `.md`
   succeeds.

Run Shell COM operations on a deliberately initialized STA and release every
COM object after the result is known. Implement complete vtable prefixes for
`IAssocHandler`, `IEnumAssocHandlers`, `IShellItem`, and
`IObjectWithAppUserModelID`; a signature that merely compiles is not ABI proof.
Do not use `MakeDefault`, `SetAppID`, default-app UI, `UserChoice`, or manually
written registration as part of this probe. The chosen operation is enumeration
plus invoking an existing handler, not changing the default.

### Interpreting the result

- Exact handler found and document opens: original `ActivateForFile` test path
  was inappropriate for this package; keep the genuine Shell invocation test.
- No exact handler: investigate per-user registration, actual installed manifest,
  and Shell registration timing; do not replace the check with a direct EXE
  launch and call it file-association coverage.
- Handler found but invocation fails: retain exact HRESULT, enumerated identity,
  and AppModel activation events; test a single manifest variant next, rather
  than blindly adding capabilities or taking defaults.
- Merely launching the installed EXE with a path can be a process-start control,
  but cannot prove FTA registration or Open With activation.

Windows Server runner results remain supplemental to supported Windows client
testing. This probe does not validate Store delivery, keyboard accessibility,
or changing the user's defaults.

## Primary sources

1. [IApplicationActivationManager::ActivateForFile](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-iapplicationactivationmanager-activateforfile)
2. [Integrate your packaged desktop app with Windows using packaging extensions](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/desktop-to-uwp-extensions), sections "Associate your packaged application with a set of file types" and "Add options to the context menus of files that have a certain file type".
3. [uap3:FileTypeAssociation schema](https://learn.microsoft.com/en-us/uwp/schemas/appxpackage/uapmanifestschema/element-uap3-filetypeassociation)
4. [uap3:Verb schema](https://learn.microsoft.com/en-us/uwp/schemas/appxpackage/uapmanifestschema/element-uap3-verb) and [uap2:SupportedVerbs schema](https://learn.microsoft.com/en-us/uwp/schemas/appxpackage/uapmanifestschema/element-uap2-supportedverbs)
5. [Application schema](https://learn.microsoft.com/en-us/uwp/schemas/appxpackage/uapmanifestschema/element-application), "Combinations of activation info attributes".
6. [SHAssocEnumHandlers](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-shassocenumhandlers)
7. [IObjectWithAppUserModelID](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-iobjectwithappusermodelid)
8. [IAssocHandler::GetName](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-iassochandler-getname)
9. [IShellItem::BindToHandler](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishellitem-bindtohandler)
10. [IAssocHandler::Invoke](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-iassochandler-invoke)
