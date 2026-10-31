param(
    [Parameter(Mandatory=$true)][string]$Output,
    [string]$Version = '1.0.14.0',
    [string]$Identity = '',
    [switch]$TestIdentity,
    [string]$MakeAppx = 'makeappx.exe',
    [string]$Python = 'python'
)
$ErrorActionPreference='Stop'
if ([bool]$Identity -eq [bool]$TestIdentity) { throw 'Choose either -Identity or -TestIdentity' }
$root=Split-Path $PSScriptRoot
$outputPath=[IO.Path]::GetFullPath($Output)
if(Test-Path -LiteralPath $outputPath){throw 'Output must be a new directory'}
Push-Location $root
try {
    & cargo +stable-x86_64-pc-windows-msvc build --release --locked --features store --target-dir target/store
    if($LASTEXITCODE){throw 'Store build failed'}
    $args=@('packaging/msix.py','--binary','target/store/release/feathermd.exe','--output',$outputPath,'--version',$Version,'--makeappx',$MakeAppx)
    if($TestIdentity){$args+='--test-identity'}else{$args+=@('--identity',$Identity)}
    & $Python @args
    if($LASTEXITCODE){throw 'MSIX packaging failed'}
} finally { Pop-Location }
