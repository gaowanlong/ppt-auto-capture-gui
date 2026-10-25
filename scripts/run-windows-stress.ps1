[CmdletBinding()]
param(
    [ValidateRange(1, 1440)][int]$Minutes = 10,
    [ValidateSet('720p', '1080p', '4k')][string]$Resolution = '1080p',
    [ValidateRange(0, 1000)][int]$IntervalMilliseconds = 100,
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '..\.artifacts\windows-stress'),
    [string]$TestExecutable = ''
)

$ErrorActionPreference = 'Stop'
$runName = (Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + [guid]::NewGuid().ToString('N')
$reportDirectory = [IO.Path]::GetFullPath((Join-Path $OutputDirectory $runName))
New-Item -ItemType Directory -Path $reportDirectory -Force | Out-Null
$nativeReport = Join-Path $reportDirectory 'native-report.json'
$logPath = Join-Path $reportDirectory 'stress.log'
$dimensions = @{ '720p' = @(1280, 720); '1080p' = @(1920, 1080); '4k' = @(3840, 2160) }[$Resolution]
$settings = @{
    PPT_CAPTURE_STRESS_SECONDS = [string]($Minutes * 60)
    PPT_CAPTURE_STRESS_WIDTH = [string]$dimensions[0]
    PPT_CAPTURE_STRESS_HEIGHT = [string]$dimensions[1]
    PPT_CAPTURE_STRESS_INTERVAL_MS = [string]$IntervalMilliseconds
    PPT_CAPTURE_STRESS_REPORT = $nativeReport
}
$previous = @{}
$exitCode = 1
$failure = $null
$started = [DateTime]::UtcNow
try {
    if ($env:OS -ne 'Windows_NT') { throw 'This stress test requires Windows.' }
    if (-not $TestExecutable) {
        $bundled = Join-Path $PSScriptRoot 'ppt-capture-stress-tests.exe'
        if (Test-Path -LiteralPath $bundled) { $TestExecutable = $bundled }
    }
    foreach ($name in $settings.Keys) {
        $previous[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
        [Environment]::SetEnvironmentVariable($name, $settings[$name], 'Process')
    }
    Write-Host "Testing a controlled Windows window for $Minutes minute(s) at $Resolution. Do not close the test window."
    Write-Host "Report directory: $reportDirectory"
    if ($TestExecutable) {
        $command = (Resolve-Path -LiteralPath $TestExecutable).Path
        $arguments = @('windows_native_capture_stress_soak', '--ignored', '--nocapture', '--test-threads=1')
    } else {
        $command = (Get-Command cargo -ErrorAction Stop).Source
        $manifest = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\Cargo.toml'))
        $arguments = @('test', '--manifest-path', $manifest, '--release', 'windows_native_capture_stress_soak', '--', '--ignored', '--nocapture', '--test-threads=1')
    }
    # Native stderr (including normal compiler output) must not abort Windows PowerShell 5.
    $ErrorActionPreference = 'Continue'
    & $command @arguments 2>&1 | Tee-Object -FilePath $logPath
    $exitCode = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($exitCode -ne 0) { throw "Native stress test failed with exit code $exitCode. See $logPath" }
    if (-not (Test-Path -LiteralPath $nativeReport)) { throw 'The executable produced no native stress report; the test may not have run.' }
    $report = Get-Content -LiteralPath $nativeReport -Raw | ConvertFrom-Json
    if ($report.outcome -ne 'passed' -or $report.frames -lt 10 -or $report.changed_frames -lt 1 -or $report.requested_seconds -ne ($Minutes * 60) -or $report.elapsed_seconds -lt ($Minutes * 60)) { throw 'Native stress report did not confirm the requested duration and changing captured frames.' }
    Write-Host "PASS: $($report.frames) frames in $([math]::Round($report.elapsed_seconds, 2)) seconds."
    $exitCode = 0
} catch {
    $failure = $_.Exception.Message
    $exitCode = 1
    Write-Host "FAIL: $failure"
} finally {
    foreach ($name in $previous.Keys) { [Environment]::SetEnvironmentVariable($name, $previous[$name], 'Process') }
    @{
        outcome = $(if ($exitCode -eq 0) { 'passed' } else { 'failed' })
        requested_minutes = $Minutes
        resolution = $Resolution
        interval_milliseconds = $IntervalMilliseconds
        started_utc = $started.ToString('o')
        finished_utc = [DateTime]::UtcNow.ToString('o')
        exit_code = $exitCode
        error = $failure
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $reportDirectory 'runner-result.json') -Encoding UTF8
}
exit $exitCode
