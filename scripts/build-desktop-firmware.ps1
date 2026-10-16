# Builds the complete catalog firmware set and embeds it in the desktop binary. Never flashes.
[CmdletBinding()]
param([switch]$Release)

$ErrorActionPreference = 'Stop'
$kivoriRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$kivoriPreviousFirmware = $env:KIVORI_FIRMWARE_PATH
$kivoriPreviousManifest = $env:KIVORI_FIRMWARE_MANIFEST
$kivoriPreviousTarget = $env:CARGO_TARGET_DIR
$kivoriStaging = Join-Path ([System.IO.Path]::GetTempPath()) ('kivori-firmware-' + [guid]::NewGuid().ToString('N'))
try {
    New-Item -ItemType Directory -Path $kivoriStaging | Out-Null
    $env:KIVORI_FIRMWARE_PATH = $null
    $kivoriManifest = & bun (Join-Path $kivoriRoot 'scripts/build-firmware-set.mjs') `
        --out-dir (Join-Path $kivoriStaging 'images') --manifest-out (Join-Path $kivoriStaging 'manifest.json')
    if ($LASTEXITCODE -ne 0) { throw 'Catalog firmware set build failed.' }
    if (@($kivoriManifest).Count -ne 1 -or [string]::IsNullOrWhiteSpace($kivoriManifest) -or
        -not [System.IO.Path]::IsPathRooted($kivoriManifest) -or
        -not (Test-Path -LiteralPath $kivoriManifest -PathType Leaf)) {
        throw 'Firmware builder must return exactly one existing absolute manifest path.'
    }
    $env:KIVORI_FIRMWARE_MANIFEST = $kivoriManifest
    $env:CARGO_TARGET_DIR = Join-Path $kivoriRoot 'target'
    Push-Location $kivoriRoot
    try {
        $kivoriBuildArgs = @('build', '--locked', '-p', 'kivori-desktop')
        if ($Release) { $kivoriBuildArgs += @('--release', '--no-default-features') }
        & cargo @kivoriBuildArgs
        if ($LASTEXITCODE -ne 0) { throw 'Desktop build failed.' }
    } finally { Pop-Location }
    Write-Host 'Desktop built with the complete catalog firmware set. No device was flashed.'
} finally {
    $env:KIVORI_FIRMWARE_PATH = $kivoriPreviousFirmware
    $env:KIVORI_FIRMWARE_MANIFEST = $kivoriPreviousManifest
    $env:CARGO_TARGET_DIR = $kivoriPreviousTarget
    if (Test-Path -LiteralPath $kivoriStaging) { Remove-Item -LiteralPath $kivoriStaging -Recurse -Force }
}
