# Builds catalog-selected firmware and embeds it in the desktop binary. Does not flash a device.
[CmdletBinding()]
param([switch]$Release, [string]$Product, [string]$Board)

$ErrorActionPreference = 'Stop'
$kivoriRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$kivoriPreviousFirmware = $env:KIVORI_FIRMWARE_PATH
$kivoriPreviousTarget = $env:CARGO_TARGET_DIR
try {
    $kivoriFirmwareArgs = @((Join-Path $kivoriRoot 'scripts/build-firmware.mjs'))
    if ($Product) { $kivoriFirmwareArgs += @('--product', $Product) }
    if ($Board) { $kivoriFirmwareArgs += @('--board', $Board) }
    $kivoriFirmwarePath = & bun @kivoriFirmwareArgs
    if ($LASTEXITCODE -ne 0) { throw 'Catalog firmware build failed.' }
    if (@($kivoriFirmwarePath).Count -ne 1 -or -not (Test-Path -LiteralPath $kivoriFirmwarePath -PathType Leaf)) {
        throw 'Firmware builder must return exactly one existing artifact path.'
    }
    $env:KIVORI_FIRMWARE_PATH = $kivoriFirmwarePath
    $env:CARGO_TARGET_DIR = Join-Path $kivoriRoot 'target'
    Push-Location $kivoriRoot
    try {
        $kivoriBuildArgs = @('build', '--locked', '-p', 'kivori-desktop')
        if ($Release) { $kivoriBuildArgs += @('--release', '--no-default-features') }
        & cargo @kivoriBuildArgs
        if ($LASTEXITCODE -ne 0) { throw 'Desktop build failed.' }
    } finally { Pop-Location }
    Write-Host 'Desktop built with catalog-selected firmware. No device was flashed.'
} finally {
    $env:KIVORI_FIRMWARE_PATH = $kivoriPreviousFirmware
    $env:CARGO_TARGET_DIR = $kivoriPreviousTarget
}
