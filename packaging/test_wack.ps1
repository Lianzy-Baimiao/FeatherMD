param(
    [Parameter(Mandatory=$true)][string]$Package,
    [Parameter(Mandatory=$true)][string]$PackageReport,
    [Parameter(Mandatory=$true)][string]$Output
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'WACK changes trust and installs software. Run only on a disposable GitHub-hosted Windows runner.'
}
$Package = (Resolve-Path -LiteralPath $Package).Path
$PackageReport = (Resolve-Path -LiteralPath $PackageReport).Path
$build = Get-Content -LiteralPath $PackageReport -Raw | ConvertFrom-Json
$name = 'FeatherMD.StoreTest'
$publisher = 'CN=FeatherMD Store Test'
if ($build.test_only -ne $true -or $build.identity.name -ne $name -or $build.identity.publisher -ne $publisher) {
    throw 'This supplemental workflow accepts only FeatherMD test-identity packages.'
}
if ($build.package_sha256 -notmatch '^[0-9a-f]{64}$' -or $build.binary_sha256 -notmatch '^[0-9a-f]{64}$') {
    throw 'Missing build integrity hashes.'
}
$Output = [IO.Path]::GetFullPath($Output)
$prefix = [IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\') + '\'
if (-not $Output.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $Output)) {
    throw 'Reports must use a new RUNNER_TEMP directory.'
}
if (Get-AppxPackage -Name $name) { throw 'Refusing to disturb an existing test package.' }
if (@(Get-ChildItem Env: | Where-Object Name -Like 'FEATHERMD_TEST_*').Count) {
    throw 'Test overrides must not contaminate WACK.'
}
New-Item -ItemType Directory -Path $Output | Out-Null
# Private signing material and signed copies are never uploaded with reports.
$private = Join-Path $env:RUNNER_TEMP ('feathermd-wack-private-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $private | Out-Null
$cert = $null
$mayBeInstalled = $false
$state = [ordered]@{ status='inconclusive'; stage='preflight'; error=$null; cleanup_errors=@()
    scope='Supplemental Windows Server runner, test identity; not Windows 10/11 acceptance or Store certification'
    original_msix_sha256=$build.package_sha256; binary_sha256=$build.binary_sha256; signed_copy_sha256=$null }
function Save-Json($Value, [string]$Name) {
    $Value | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $Output $Name) -Encoding utf8
}
function Assert-Hash([string]$Path, [string]$Expected) {
    if ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $Expected.ToLowerInvariant()) {
        throw "Integrity mismatch: $Path"
    }
}
function Microsoft-Signature([string]$Path) {
    $s = Get-AuthenticodeSignature -LiteralPath $Path
    if ($s.Status -ne 'Valid' -or $s.SignerCertificate.Subject -notmatch '(?:^|,\s*)O=Microsoft Corporation(?:,|$)') {
        throw "Expected valid Microsoft signature: $Path"
    }
    return @{ status=[string]$s.Status; subject=$s.SignerCertificate.Subject; sha256=(Get-FileHash -LiteralPath $Path).Hash }
}
function Run([string]$File, [string]$Arguments, [string]$Label, [int]$Timeout) {
    $record = [ordered]@{ file=$File; arguments=$Arguments; started_utc=[DateTime]::UtcNow.ToString('o')
        timeout_seconds=$Timeout; timed_out=$false; exit_code=$null }
    $process = $null
    try {
        $process = Start-Process -FilePath $File -ArgumentList $Arguments -PassThru -WindowStyle Hidden `
            -RedirectStandardOutput (Join-Path $Output "$Label.stdout.log") -RedirectStandardError (Join-Path $Output "$Label.stderr.log")
        $null = $process.Handle
        if (-not $process.WaitForExit($Timeout * 1000)) {
            $record.timed_out = $true
            & taskkill.exe /PID $process.Id /T /F | Out-Host
            throw "Timed out: $Label"
        }
        $record.exit_code = $process.ExitCode
        if ($null -eq $record.exit_code) { throw "Missing exit code: $Label" }
        return $record.exit_code
    } finally {
        $record['finished_utc'] = [DateTime]::UtcNow.ToString('o')
        Save-Json $record "$Label.command.json"
        if ($process) { $process.Dispose() }
    }
}
try {
    Assert-Hash $Package $build.package_sha256
    $os = Get-CimInstance Win32_OperatingSystem
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $admin = ([Security.Principal.WindowsPrincipal]$identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    Save-Json @{ os=$os.Caption; version=$os.Version; build=$os.BuildNumber; administrator=$admin
        image_version=$env:ImageVersion; commit=$env:GITHUB_SHA; run_id=$env:GITHUB_RUN_ID } 'environment.json'
    if (-not $admin) { throw 'WACK CLI requires administrator rights on the disposable runner.' }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $payload = Join-Path $private 'payload'
    [IO.Compression.ZipFile]::ExtractToDirectory($Package, $payload)
    Assert-Hash (Join-Path $payload 'FeatherMD.exe') $build.binary_sha256
    $manifest = Join-Path $payload 'AppxManifest.xml'
    [xml]$xml = Get-Content -LiteralPath $manifest -Raw
    if ($xml.Package.Identity.Name -ne $name -or $xml.Package.Identity.Publisher -ne $publisher -or
        $xml.Package.Identity.Version -ne $build.version -or $xml.Package.Identity.ProcessorArchitecture -ne 'x64') {
        throw 'Package manifest does not match build report.'
    }
    Copy-Item -LiteralPath $manifest -Destination (Join-Path $Output 'AppxManifest.xml')
    $state.stage = 'tool-installation'
    $appcert = "${env:ProgramFiles(x86)}\Windows Kits\10\App Certification Kit\appcert.exe"
    if (-not (Test-Path -LiteralPath $appcert)) {
        $installer = Join-Path $private 'winsdksetup.exe'
        Invoke-WebRequest -Uri 'https://download.microsoft.com/download/46742ab5-6592-4968-a793-129e7f3bc55a/KIT_BUNDLE_WINDOWSSDK_MEDIACREATION/winsdksetup.exe' -OutFile $installer -TimeoutSec 180
        Assert-Hash $installer 'b0bdbad38ae74c40ccae8cdbcf229807627fba2bc3100be28301c4ba884f755e'
        Save-Json (Microsoft-Signature $installer) 'sdk-installer-signature.json'
        $code = Run $installer ('/features OptionId.WindowsSoftwareLogoToolkit /quiet /norestart /log "' + (Join-Path $Output 'sdk-install.log') + '"') 'sdk-install' 900
        if ($code -ne 0) { throw "SDK installation failed or needs reboot: $code" }
    }
    if (-not (Test-Path -LiteralPath $appcert)) { throw 'appcert.exe is missing.' }
    Save-Json @{ version=(Get-Item -LiteralPath $appcert).VersionInfo.FileVersion; signature=(Microsoft-Signature $appcert) } 'appcert-version.json'
    $sign = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" | Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $sign) { throw 'SignTool is missing.' }
    $state.stage = 'test-signature'
    $cert = New-SelfSignedCertificate -Type Custom -Subject $publisher -KeyUsage DigitalSignature -KeyExportPolicy NonExportable `
        -CertStoreLocation Cert:\CurrentUser\My -NotAfter (Get-Date).AddDays(2) -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3')
    $cer = Join-Path $private 'test.cer'
    Export-Certificate -Cert $cert -FilePath $cer | Out-Null
    Import-Certificate -FilePath $cer -CertStoreLocation Cert:\LocalMachine\TrustedPeople | Out-Null
    $signed = Join-Path $private 'FeatherMD-test-only.msix'
    Copy-Item -LiteralPath $Package -Destination $signed
    $code = Run $sign.FullName ('sign /fd SHA256 /sha1 ' + $cert.Thumbprint + ' "' + $signed + '"') 'sign' 120
    if ($code -ne 0) { throw "Test signing failed: $code" }
    $code = Run $sign.FullName ('verify /pa /v "' + $signed + '"') 'verify-signature' 120
    if ($code -ne 0) { throw "Test signature verification failed: $code" }
    $signedPayload = Join-Path $private 'signed-payload'
    [IO.Compression.ZipFile]::ExtractToDirectory($signed, $signedPayload)
    Assert-Hash (Join-Path $signedPayload 'FeatherMD.exe') $build.binary_sha256
    Assert-Hash (Join-Path $signedPayload 'AppxManifest.xml') (Get-FileHash -LiteralPath $manifest).Hash
    $state.signed_copy_sha256 = (Get-FileHash -LiteralPath $signed).Hash
    $state.stage = 'wack'
    $code = Run $appcert 'reset' 'appcert-reset' 120
    if ($code -ne 0) { throw "WACK reset failed: $code" }
    $mayBeInstalled = $true
    $report = Join-Path $Output 'wack.xml'
    $code = Run $appcert ('test -appxpackagepath "' + $signed + '" -reportoutputpath "' + $report + '"') 'appcert-test' 1800
    $state.stage = 'report-review'
    & python (Join-Path $PSScriptRoot 'wack_report.py') --report $report "--exit-code=$code" --output (Join-Path $Output 'wack-summary.json')
    $summaryCode = $LASTEXITCODE
    $summary = Get-Content -LiteralPath (Join-Path $Output 'wack-summary.json') -Raw | ConvertFrom-Json
    $state.status = $summary.status
    if ($summaryCode -ne 0) { throw "WACK $($summary.status): $($summary.reason)" }
    $state.stage = 'completed'
} catch {
    $state.error = $_.Exception.Message
    throw
} finally {
    if ($mayBeInstalled) {
        try { Get-AppxPackage -Name $name | Where-Object Publisher -eq $publisher | Remove-AppxPackage }
        catch { $state.cleanup_errors += $_.Exception.Message }
    }
    if ($cert) {
        foreach ($store in @('Cert:\LocalMachine\TrustedPeople', 'Cert:\CurrentUser\My')) {
            try { Remove-Item -LiteralPath "$store\$($cert.Thumbprint)" -ErrorAction Stop }
            catch { $state.cleanup_errors += $_.Exception.Message }
        }
    }
    try { Assert-Hash $Package $build.package_sha256 }
    catch { $state.cleanup_errors += $_.Exception.Message }
    if ($state.cleanup_errors.Count) { $state.status = 'inconclusive' }
    Save-Json $state 'validation.json'
    if ($env:GITHUB_STEP_SUMMARY) {
        @('# Supplemental WACK validation', '', "Status: $($state.status)", "Stage: $($state.stage)", '', $state.scope,
            '', 'Optional failures remain in raw reports and require review. No test certificates, private keys or signed copies are uploaded.') |
            Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY
    }
    if ($state.cleanup_errors.Count) { throw ('Cleanup/integrity failed: ' + ($state.cleanup_errors -join '; ')) }
}
