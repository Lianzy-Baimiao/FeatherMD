param([switch]$AllowDesktopUi, [switch]$SecondaryMonitor, [string]$Exe)
$hosted = $env:GITHUB_ACTIONS -eq 'true' -and $env:RUNNER_ENVIRONMENT -eq 'github-hosted' -and $env:RUNNER_OS -eq 'Windows'
if ($SecondaryMonitor) {
    Add-Type -AssemblyName System.Windows.Forms
    if (@([Windows.Forms.Screen]::AllScreens | Where-Object { -not $_.Primary }).Count -ne 1) {
        throw 'Expected exactly one secondary monitor. No test application was started.'
    }
    $bytes=[IO.File]::ReadAllBytes((Resolve-Path -LiteralPath $Exe).Path)
    if(-not [Text.Encoding]::ASCII.GetString($bytes).Contains('FEATHERMD_SECONDARY_TEST_V1')) {
        throw 'Rebuild first: this EXE cannot safely contain test windows on the secondary monitor.'
    }
    $env:FEATHERMD_TEST_SECONDARY_MONITOR='1'
} elseif (-not $hosted -and -not $AllowDesktopUi) {
    throw 'Native UI tests can show dialogs even with WindowStyle Hidden. Use a disposable Windows runner, or -SecondaryMonitor with explicit user approval. -AllowDesktopUi permits the primary desktop and requires separate approval.'
}
