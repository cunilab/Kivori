# Hardware-free wrapper contract. bun/cargo are mocked functions; no actual build or installer runs.
$ErrorActionPreference = 'Stop'
$kivoriTestRoot = Join-Path ([IO.Path]::GetTempPath()) ('kivori packaging test ' + [guid]::NewGuid().ToString('N'))
$kivoriWrapper = Join-Path $PSScriptRoot 'build-desktop-firmware.ps1'
$kivoriSavedTemp = $env:TEMP
$kivoriSavedTmp = $env:TMP
$kivoriSavedPath = $env:KIVORI_FIRMWARE_PATH
$kivoriSavedManifest = $env:KIVORI_FIRMWARE_MANIFEST
$kivoriSavedTarget = $env:CARGO_TARGET_DIR
function global:Assert-Kivori($condition, $message) { if (-not $condition) { throw $message } }
function global:bun {
    $global:kivoriTestCalls += 'bun'
    Assert-Kivori ($null -eq $env:KIVORI_FIRMWARE_PATH) 'Legacy firmware input reached set builder.'
    Assert-Kivori ($args[0] -like '*build-firmware-set.mjs') 'Wrong firmware builder.'
    $index = [array]::IndexOf([object[]]$args, '--manifest-out')
    Assert-Kivori ($index -ge 0) 'Missing manifest output argument.'
    $global:kivoriTestManifest = $args[$index + 1]
    Set-Content -LiteralPath $global:kivoriTestManifest -Value '[]'
    $global:LASTEXITCODE = 0
    switch ($global:kivoriTestMode) {
        'helper-failure' { $global:LASTEXITCODE = 2; $global:kivoriTestManifest; return }
        'empty-output' { return }
        'multiple-lines' { $global:kivoriTestManifest; $global:kivoriTestManifest; return }
        'missing-file' { Join-Path (Split-Path $global:kivoriTestManifest) 'missing.json'; return }
        'relative-file' { 'relative.json'; return }
        default { $global:kivoriTestManifest }
    }
}
function global:cargo {
    $global:kivoriTestCalls += 'cargo'
    Assert-Kivori ($null -eq $env:KIVORI_FIRMWARE_PATH) 'Legacy firmware input reached cargo.'
    Assert-Kivori ($env:KIVORI_FIRMWARE_MANIFEST -eq $global:kivoriTestManifest) 'Wrong manifest reached cargo.'
    Assert-Kivori (Test-Path -LiteralPath $env:KIVORI_FIRMWARE_MANIFEST -PathType Leaf) 'Manifest disappeared before cargo.'
    Assert-Kivori ([IO.Path]::IsPathRooted($env:CARGO_TARGET_DIR)) 'Desktop target directory must be absolute.'
    $expected = if ($global:kivoriTestMode -eq 'debug') { 'build --locked -p kivori-desktop' } else { 'build --locked -p kivori-desktop --release --no-default-features' }
    Assert-Kivori (($args -join ' ') -eq $expected) 'Desktop build flags changed.'
    $global:LASTEXITCODE = if ($global:kivoriTestMode -eq 'cargo-failure') { 3 } else { 0 }
}
try {
    New-Item -ItemType Directory -Path $kivoriTestRoot | Out-Null
    $env:TEMP = $kivoriTestRoot
    $env:TMP = $kivoriTestRoot
    foreach ($mode in @('success', 'debug', 'helper-failure', 'empty-output', 'multiple-lines', 'missing-file', 'relative-file', 'cargo-failure')) {
        $env:KIVORI_FIRMWARE_PATH = 'previous image'
        $env:KIVORI_FIRMWARE_MANIFEST = 'previous manifest'
        $env:CARGO_TARGET_DIR = 'previous target'
        $global:kivoriTestMode = $mode
        $global:kivoriTestCalls = @()
        $global:kivoriTestManifest = $null
        $failed = $false
        $failureMessage = ''
        $before = (Get-Location).Path
        try { & $kivoriWrapper -Release:($mode -ne 'debug') } catch { $failed = $true; $failureMessage = $_.Exception.Message }
        Assert-Kivori ($failed -eq ($mode -notin @('success', 'debug'))) "Unexpected result: $mode"
        if ($failed) {
            $expectedFailure = switch ($mode) {
                'helper-failure' { 'Catalog firmware set build failed.' }
                'cargo-failure' { 'Desktop build failed.' }
                default { 'Firmware builder must return exactly one existing absolute manifest path.' }
            }
            Assert-Kivori ($failureMessage -eq $expectedFailure) "Unexpected failure in ${mode}: $failureMessage"
        }
        $expectedCalls = if ($mode -in @('success', 'debug', 'cargo-failure')) { 'bun,cargo' } else { 'bun' }
        Assert-Kivori (($global:kivoriTestCalls -join ',') -eq $expectedCalls) "Wrong calls: $mode"
        Assert-Kivori ($env:KIVORI_FIRMWARE_PATH -eq 'previous image') "Legacy env not restored: $mode"
        Assert-Kivori ($env:KIVORI_FIRMWARE_MANIFEST -eq 'previous manifest') "Manifest env not restored: $mode"
        Assert-Kivori ($env:CARGO_TARGET_DIR -eq 'previous target') "Target env not restored: $mode"
        Assert-Kivori ((Get-Location).Path -eq $before) "Working directory not restored: $mode"
        Assert-Kivori (-not (Test-Path -LiteralPath (Split-Path $global:kivoriTestManifest))) "Staging directory not removed: $mode"
        Write-Host "PASS packaging wrapper: $mode"
    }
    # GitHub's pwsh wrapper forwards the last native exit code.
    $global:LASTEXITCODE = 0
} finally {
    $env:TEMP = $kivoriSavedTemp
    $env:TMP = $kivoriSavedTmp
    $env:KIVORI_FIRMWARE_PATH = $kivoriSavedPath
    $env:KIVORI_FIRMWARE_MANIFEST = $kivoriSavedManifest
    $env:CARGO_TARGET_DIR = $kivoriSavedTarget
    Remove-Item Function:\bun, Function:\cargo, Function:\Assert-Kivori -ErrorAction SilentlyContinue
    Remove-Variable kivoriTestMode, kivoriTestCalls, kivoriTestManifest -Scope Global -ErrorAction SilentlyContinue
    if (Test-Path -LiteralPath $kivoriTestRoot) { Remove-Item -LiteralPath $kivoriTestRoot -Recurse -Force }
}
