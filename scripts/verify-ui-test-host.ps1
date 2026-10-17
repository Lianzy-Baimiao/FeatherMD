param([string]$Output, [switch]$SecondaryMonitor)
if (-not $SecondaryMonitor) { return }
$report=Join-Path $Output 'test-host.txt'
if(-not (Test-Path -LiteralPath $report)){throw 'Missing secondary-monitor containment evidence'}
$lines=[IO.File]::ReadAllLines($report)
if(-not $lines.Count){throw 'Empty secondary-monitor report'}
foreach($line in $lines){
    if($line -notmatch 'windows=[1-9][0-9]* .*violations=0$') {
        throw "Window containment or foreground activation failure: $line"
    }
}
Write-Output "Secondary-monitor containment verified for $($lines.Count) test process(es); zero boundary/foreground violations."
