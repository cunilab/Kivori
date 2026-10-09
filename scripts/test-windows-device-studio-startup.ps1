# Defaults to the debug Device Studio build. The release workflow passes the installed exe and
# `--autostart` to prove the shipped app starts hidden and stays alive.
[CmdletBinding()]
param(
    [string]$Exe,
    [Alias('Args')]
    [string[]]$AppArgs = @()
)

$ErrorActionPreference = 'Stop'

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$exe = if ($Exe) { $Exe } else { Join-Path $repoRoot 'target\debug\kivori-desktop.exe' }

if (-not (Test-Path $exe)) {
    throw "Binary not found at $exe"
}

$tempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
$stdout = Join-Path $tempRoot 'kivori-device-studio-stdout.log'
$stderr = Join-Path $tempRoot 'kivori-device-studio-stderr.log'
Remove-Item $stdout, $stderr -ErrorAction SilentlyContinue

Write-Host "Launching $exe"
$startArgs = @{ FilePath = $exe; PassThru = $true; RedirectStandardOutput = $stdout; RedirectStandardError = $stderr }
if ($AppArgs.Count -gt 0) { $startArgs.ArgumentList = $AppArgs }
$process = Start-Process @startArgs

try {
    Start-Sleep -Seconds 10
    $process.Refresh()

    $outText = if (Test-Path $stdout) { Get-Content $stdout -Raw } else { '' }
    $errText = if (Test-Path $stderr) { Get-Content $stderr -Raw } else { '' }

    if ($process.HasExited) {
        Write-Host '=== Device Studio stdout ==='
        Write-Host $outText
        Write-Host '=== Device Studio stderr ==='
        Write-Host $errText
        throw "Device Studio exited during the 10-second startup window with code $($process.ExitCode)"
    }

    if ($errText -match '(?i)stack overflow|fatal runtime error') {
        Write-Host '=== Device Studio stderr ==='
        Write-Host $errText
        throw 'Device Studio emitted a stack-overflow/fatal-runtime signature during startup'
    }

    Write-Host 'KIVORI-WINDOWS-STARTUP PASS: process remained alive for 10 seconds with no fatal runtime signature.'
}
finally {
    $process.Refresh()
    if (-not $process.HasExited) {
        Stop-Process -Id $process.Id -Force
        Wait-Process -Id $process.Id -ErrorAction SilentlyContinue
    }
}
