# Fetches the pinned espflash release and places it where Tauri's `externalBin` expects it:
#   apps\desktop\src-tauri\binaries\espflash-<target-triple>.exe
# (M3 S5.) Windows x64 only; macOS uses scripts/fetch-espflash.sh. The SHA-256 is pinned below and
# checked before extraction (keep in sync with fetch-espflash.sh).
[CmdletBinding()]
param([string]$Triple = 'x86_64-pc-windows-msvc')

$ErrorActionPreference = 'Stop'

$espflashVersion = '4.5.0'
$assets = @{
    'x86_64-pc-windows-msvc' = @{
        Name   = 'espflash-x86_64-pc-windows-msvc.zip'
        Sha256 = '854c82c947c20e7f337f120f0398042ed1760f2269cb0f9be3d2beae3b66fbfb'
    }
}
if (-not $assets.ContainsKey($Triple)) {
    throw "No pinned espflash asset for target '$Triple' (supported: $($assets.Keys -join ', '))"
}
$asset = $assets[$Triple]

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$destDir = Join-Path $repoRoot 'apps\desktop\src-tauri\binaries'
$dest = Join-Path $destDir "espflash-$Triple.exe"
$work = Join-Path ([System.IO.Path]::GetTempPath()) ("kivori-espflash-" + [System.Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null

try {
    $zip = Join-Path $work $asset.Name
    $url = "https://github.com/esp-rs/espflash/releases/download/v$espflashVersion/$($asset.Name)"
    Write-Host "Fetching espflash $espflashVersion ($($asset.Name))..."
    Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing

    $actual = (Get-FileHash -Algorithm SHA256 -Path $zip).Hash.ToLowerInvariant()
    if ($actual -ne $asset.Sha256) {
        throw "SHA-256 mismatch for $($asset.Name): expected $($asset.Sha256), actual $actual"
    }
    Write-Host 'SHA-256 verified.'

    $extract = Join-Path $work 'x'
    Expand-Archive -Path $zip -DestinationPath $extract
    $exe = Join-Path $extract 'espflash.exe'
    if (-not (Test-Path $exe)) { throw "espflash.exe not found in $($asset.Name)" }

    New-Item -ItemType Directory -Path $destDir -Force | Out-Null
    Copy-Item $exe $dest -Force
    Write-Host "Wrote $dest"
}
finally {
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
}
