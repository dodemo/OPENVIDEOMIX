# Build-host packaging script. No dependency on this script or PowerShell at runtime.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-fA-F]{40}$')]
    [string]$CommitSha
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repository = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$executable = Join-Path $repository 'target\release\openvideomix.exe'
if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
    throw 'Release executable does not exist: target\release\openvideomix.exe'
}

# Verify a real 64-bit Windows PE executable, not a text file or wrong architecture.
$stream = [System.IO.File]::OpenRead($executable)
try {
    $reader = [System.IO.BinaryReader]::new($stream)
    if ($reader.ReadUInt16() -ne 0x5A4D) { throw 'Executable has no MZ header' }
    [void]$stream.Seek(0x3C, [System.IO.SeekOrigin]::Begin)
    $peOffset = $reader.ReadInt32()
    if ($peOffset -lt 64 -or $peOffset -gt ($stream.Length - 6)) {
        throw 'Executable has an invalid PE header offset'
    }
    [void]$stream.Seek($peOffset, [System.IO.SeekOrigin]::Begin)
    if ($reader.ReadUInt32() -ne 0x00004550) { throw 'Executable has no PE signature' }
    if ($reader.ReadUInt16() -ne 0x8664) { throw 'Executable is not Windows x64' }
}
finally {
    $stream.Dispose()
}

Push-Location -LiteralPath $repository
try {
    $cargoJson = & cargo metadata --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
    $packages = ($cargoJson | ConvertFrom-Json).packages
    $package = $packages | Where-Object { $_.name -eq 'openvideomix-desktop' } | Select-Object -First 1
    if ($null -eq $package -or [string]::IsNullOrWhiteSpace($package.version)) {
        throw 'Could not determine the desktop package version'
    }
    $version = [string]$package.version
}
finally {
    Pop-Location
}

$shortCommit = $CommitSha.Substring(0, 12).ToLowerInvariant()
$folderName = "OPENVIDEOMIX-Windows-Demo-x64-$version-$shortCommit"
$dist = Join-Path $repository 'dist'
$packageDirectory = Join-Path $dist $folderName
$archive = Join-Path $dist "$folderName.zip"
$checksum = Join-Path $dist "$folderName.zip.sha256"

New-Item -ItemType Directory -Path $packageDirectory -Force | Out-Null
Copy-Item -LiteralPath $executable -Destination (Join-Path $packageDirectory 'openvideomix.exe') -Force
Copy-Item -LiteralPath (Join-Path $repository 'LICENSE') -Destination (Join-Path $packageDirectory 'LICENSE') -Force

$guide = @'
OPENVIDEOMIX - WINDOWS x64 DEMO / PROTOTIPO DE INTERFACE

1. Extrae todo o ZIP nun cartafol local do PC.
2. Fai dobre clic en openvideomix.exe.
3. O executable deste paquete NON necesita Rust, Cargo, Python nin OpenHands.
4. ESTA NON E A ALPHA AUDIOVISUAL: captura, gravacion, streaming e
   dispositivos fisicos non estan activados nesta compilacion.
5. As fontes do panel son demostracions. 50 FPS e o perfil solicitado, non unha
   medida de cadencia real. A gravacion de Program NON esta disponible.
6. Non se fixo unha proba de arranque nun Windows 11 limpo sen rede.
   Un erro de bibliotecas ou controladores debe comunicarse antes de
   considerar o paquete independente e distribuible.

Este ZIP foi compilado polo CI de GitHub. Non inclue un instalador en rede,
un modelo de IA, GStreamer, codec packs nin ferramentas de desenvolvemento.

Consulta build-manifest.json para a revision, version e SHA256 do executable.
'@
Set-Content -LiteralPath (Join-Path $packageDirectory 'LEEME.txt') -Value $guide -Encoding utf8NoBOM

$exeHash = (Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash.ToLowerInvariant()
$manifest = [ordered]@{
    product = 'OPENVIDEOMIX'
    artifact_kind = 'Windows x64 GUI-only demonstration, not audiovisual Alpha'
    version = $version
    source_commit = $CommitSha.ToLowerInvariant()
    executable = 'openvideomix.exe'
    executable_sha256 = $exeHash
    build_features = 'default (none)'
    rustflags = '-C target-feature=+crt-static'
    bundled_gstreamer = $false
    real_camera_capture = $false
    program_recording = $false
    clean_offline_windows_11_launch_tested = $false
    status = 'CI artifact; physical Windows 11 acceptance pending'
}
$manifest | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $packageDirectory 'build-manifest.json') -Encoding utf8NoBOM

if (Test-Path -LiteralPath $archive) { Remove-Item -LiteralPath $archive -Force }
Compress-Archive -LiteralPath $packageDirectory -DestinationPath $archive -CompressionLevel Optimal

# Round-trip the archive and confirm that the bytes of the executable match.
$unpacked = Join-Path $dist '_verify_demo_zip'
if (Test-Path -LiteralPath $unpacked) { Remove-Item -LiteralPath $unpacked -Recurse -Force }
try {
    Expand-Archive -LiteralPath $archive -DestinationPath $unpacked -Force
    $decodedExecutable = Join-Path (Join-Path $unpacked $folderName) 'openvideomix.exe'
    if (-not (Test-Path -LiteralPath $decodedExecutable -PathType Leaf)) {
        throw 'ZIP does not contain the expected portable executable'
    }
    $decodedHash = (Get-FileHash -LiteralPath $decodedExecutable -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($decodedHash -ne $exeHash) { throw 'ZIP executable SHA256 mismatch' }
}
finally {
    if (Test-Path -LiteralPath $unpacked) {
        Remove-Item -LiteralPath $unpacked -Recurse -Force
    }
}

$archiveHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -LiteralPath $checksum -Value "$archiveHash  $folderName.zip" -Encoding ascii
Write-Output "PASS: $folderName.zip"
Write-Output "SHA256: $archiveHash"
Write-Output "GUI-only demo, not a media-enabled offline Alpha."
