# Builds the physical mascot firmware and embeds it in the desktop binary. Does not flash a device.
[CmdletBinding()]
param([switch]$Release)

$ErrorActionPreference = 'Stop'
$kivoriRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$kivoriFirmwareRoot = Join-Path $kivoriRoot 'firmware/pala'
$kivoriPreviousFirmware = $env:KIVORI_FIRMWARE_PATH
$kivoriPreviousTarget = $env:CARGO_TARGET_DIR
try {
    # An explicit target directory makes artifact selection independent of the caller's environment.
    $env:CARGO_TARGET_DIR = Join-Path $kivoriFirmwareRoot 'target'
    Push-Location $kivoriFirmwareRoot
    try {
        & cargo build --locked --release --target riscv32imc-unknown-none-elf --no-default-features --features physical-st7789
        if ($LASTEXITCODE -ne 0) { throw 'Physical firmware build failed.' }
    } finally { Pop-Location }

    $env:KIVORI_FIRMWARE_PATH = Join-Path $env:CARGO_TARGET_DIR 'riscv32imc-unknown-none-elf/release/kivori-firmware'
    $env:CARGO_TARGET_DIR = Join-Path $kivoriRoot 'target'
    Push-Location $kivoriRoot
    try {
        $kivoriBuildArgs = @('build', '--locked', '-p', 'kivori-desktop')
        if ($Release) { $kivoriBuildArgs += @('--release', '--no-default-features') }
        & cargo @kivoriBuildArgs
        if ($LASTEXITCODE -ne 0) { throw 'Desktop build failed.' }
    } finally { Pop-Location }
    Write-Host 'Desktop built with the physical ESP32-C3/ST7789 mascot firmware. No device was flashed.'
} finally {
    $env:KIVORI_FIRMWARE_PATH = $kivoriPreviousFirmware
    $env:CARGO_TARGET_DIR = $kivoriPreviousTarget
}
