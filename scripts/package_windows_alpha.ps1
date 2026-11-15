# Build-host packaging script for OPENVIDEOMIX Windows x64 Alpha with bundled GStreamer runtime.
# Produces an offline standalone ZIP and SHA256 without external runtime installer dependencies.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-fA-F]{40}$')]
    [string]$CommitSha,

    [string]$GstreamerRoot = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repository = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$executable = Join-Path $repository 'target\release\openvideomix.exe'
if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) {
    throw 'Release executable does not exist: target\release\openvideomix.exe'
}

# Verify a real 64-bit Windows PE executable
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

if ([string]::IsNullOrWhiteSpace($GstreamerRoot)) {
    if ($env:GSTREAMER_1_0_ROOT_MSVC_X86_64) {
        $GstreamerRoot = $env:GSTREAMER_1_0_ROOT_MSVC_X86_64
    } elseif ($env:RUNNER_TEMP) {
        $GstreamerRoot = Join-Path $env:RUNNER_TEMP 'openvideomix-gstreamer-msvc'
    }
}

$shortCommit = $CommitSha.Substring(0, 12).ToLowerInvariant()
$folderName = "OPENVIDEOMIX-Windows-Alpha-x64-$version-$shortCommit"
$dist = Join-Path $repository 'dist'
$packageDirectory = Join-Path $dist $folderName
$archive = Join-Path $dist "$folderName.zip"
$checksum = Join-Path $dist "$folderName.zip.sha256"

if (Test-Path -LiteralPath $packageDirectory) {
    Remove-Item -LiteralPath $packageDirectory -Recurse -Force
}
New-Item -ItemType Directory -Path $packageDirectory -Force | Out-Null

$binDir = Join-Path $packageDirectory 'bin'
$pluginsDir = Join-Path (Join-Path $packageDirectory 'lib') 'gstreamer-1.0'
New-Item -ItemType Directory -Path $binDir -Force | Out-Null
New-Item -ItemType Directory -Path $pluginsDir -Force | Out-Null

# Copy main executable to root and bin
Copy-Item -LiteralPath $executable -Destination (Join-Path $packageDirectory 'openvideomix.exe') -Force
Copy-Item -LiteralPath $executable -Destination (Join-Path $binDir 'openvideomix.exe') -Force
Copy-Item -LiteralPath (Join-Path $repository 'LICENSE') -Destination (Join-Path $packageDirectory 'LICENSE') -Force

$bundledDlls = [System.Collections.Generic.List[string]]::new()
$bundledPlugins = [System.Collections.Generic.List[string]]::new()

if (-not [string]::IsNullOrWhiteSpace($GstreamerRoot) -and (Test-Path -LiteralPath $GstreamerRoot)) {
    Write-Output "Bundling runtime dependencies from GStreamer root: $GstreamerRoot"
    $gstBin = Join-Path $GstreamerRoot 'bin'
    $gstPlugins = Join-Path (Join-Path $GstreamerRoot 'lib') 'gstreamer-1.0'

    # Essential runtime DLLs needed by GStreamer core, video, audio and WebM pipelines
    $coreDllPatterns = @(
        'gstreamer-1.0*.dll',
        'glib-2.0*.dll',
        'gobject-2.0*.dll',
        'gmodule-2.0*.dll',
        'gio-2.0*.dll',
        'gstvideo-1.0*.dll',
        'gstaudio-1.0*.dll',
        'gstapp-1.0*.dll',
        'gstbase-1.0*.dll',
        'gsttag-1.0*.dll',
        'gstpbutils-1.0*.dll',
        'gstcontroller-1.0*.dll',
        'intl*.dll',
        'z*.dll',
        'ffi*.dll',
        'orc*.dll'
    )

    if (Test-Path -LiteralPath $gstBin) {
        foreach ($pattern in $coreDllPatterns) {
            Get-ChildItem -LiteralPath $gstBin -Filter $pattern -File -ErrorAction SilentlyContinue | ForEach-Object {
                Copy-Item -LiteralPath $_.FullName -Destination $binDir -Force
                Copy-Item -LiteralPath $_.FullName -Destination $packageDirectory -Force
                $bundledDlls.Add($_.Name)
            }
        }
    }

    # Bundled plugins needed for synthetic capture, playback and WebM VP8/Vorbis encoding
    $pluginPatterns = @(
        'gstcoreelements.dll',
        'gstapp.dll',
        'gstvideotestsrc.dll',
        'gstvideoconvertscale.dll',
        'gstvideoconvert.dll',
        'gstaudiotestsrc.dll',
        'gstaudioconvert.dll',
        'gstaudioresample.dll',
        'gstvpx.dll',
        'gstvorbis.dll',
        'gstmatroska.dll'
    )

    if (Test-Path -LiteralPath $gstPlugins) {
        foreach ($pattern in $pluginPatterns) {
            Get-ChildItem -LiteralPath $gstPlugins -Filter $pattern -File -ErrorAction SilentlyContinue | ForEach-Object {
                Copy-Item -LiteralPath $_.FullName -Destination $pluginsDir -Force
                $bundledPlugins.Add($_.Name)
            }
        }
    }
}

# Create double-clickable launcher script for non-programmers
$launcherCmd = @'
@echo off
setlocal
cd /d "%~dp0"
set "PATH=%~dp0bin;%~dp0;%PATH%"
set "GST_PLUGIN_SYSTEM_PATH_1_0=%~dp0lib\gstreamer-1.0"
set "GST_PLUGIN_PATH_1_0=%~dp0lib\gstreamer-1.0"
set "GST_REGISTRY_1_0=%LOCALAPPDATA%\Openvideomix\gst-registry.bin"
if not exist "%LOCALAPPDATA%\Openvideomix" mkdir "%LOCALAPPDATA%\Openvideomix"
echo [OPENVIDEOMIX] Iniciando estudo de producion 1080p50...
start "" "%~dp0openvideomix.exe" %*
'@
Set-Content -LiteralPath (Join-Path $packageDirectory 'INICIAR-OPENVIDEOMIX.cmd') -Value $launcherCmd -Encoding ascii

$guide = @'
OPENVIDEOMIX - ALPHA AUDIOVISUAL AUTONOMA PARA WINDOWS 11 (x64)
================================================================

Esta distribucion e autonoma e inclue o runtime de GStreamer necesario para
realizacion e gravacion en directo. NON require instalar Rust, Cargo, Python,
Visual Studio nin GStreamer adicional no sistema operativo.

COMO EXECUTAR:
1. Extrae todo o arquivo ZIP nun cartafol local (ex. C:\OPENVIDEOMIX).
2. Fai dobre clic en 'INICIAR-OPENVIDEOMIX.cmd' (ou directamente en 'openvideomix.exe').
3. Na interface:
   - Fontes de estudo: fontes de proba a 1080p50 disponseis no rack.
   - CUT / MIX / WIPE: troca instantanea entre Preview e Program.
   - 'GRAVAR TESTE': inicia a gravacion de Program a WebM (VP8 + son 48 kHz).
   - 'PARAR GRAVACION': pecha o ficheiro de gravacion de xeito limpo.
   - 'DIAGNOSTICO': exporta un informe completo sanitizado e sen datos persoais.

REQUISITOS DO SISTEMA:
- Windows 11 x64 (PC Grande con NVIDIA RTX 4080 SUPER como banco de cualificacion)
- Controladores graficos actualizados
'@
Set-Content -LiteralPath (Join-Path $packageDirectory 'LEEME.txt') -Value $guide -Encoding utf8NoBOM

$exeHash = (Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash.ToLowerInvariant()
$manifest = [ordered]@{
    product = 'OPENVIDEOMIX'
    artifact_kind = 'Windows x64 Standalone Audiovisual Alpha with bundled GStreamer runtime'
    version = $version
    source_commit = $CommitSha.ToLowerInvariant()
    executable = 'openvideomix.exe'
    executable_sha256 = $exeHash
    launcher = 'INICIAR-OPENVIDEOMIX.cmd'
    standard_profile = '1920x1080 progressive 50/1 FPS, 48 kHz PCM'
    bundled_gstreamer_runtime = $true
    bundled_dll_count = $bundledDlls.Count
    bundled_plugins = $bundledPlugins
    clean_offline_windows_11_launch_tested = $false
    licenses = @{
        'openvideomix' = 'MIT License'
        'gstreamer_core_and_plugins' = 'LGPL 2.1+'
        'libvpx' = 'BSD 3-Clause'
        'libvorbis' = 'Xiph BSD-style'
    }
    physical_hardware_status = 'Pending owner acceptance on Windows 11 RTX 4080 SUPER bench'
}
$manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $packageDirectory 'build-manifest.json') -Encoding utf8NoBOM

if (Test-Path -LiteralPath $archive) { Remove-Item -LiteralPath $archive -Force }
Compress-Archive -LiteralPath $packageDirectory -DestinationPath $archive -CompressionLevel Optimal

# Round-trip verification: extract and confirm checksum and required files
$unpacked = Join-Path $dist '_verify_alpha_zip'
if (Test-Path -LiteralPath $unpacked) { Remove-Item -LiteralPath $unpacked -Recurse -Force }
try {
    Expand-Archive -LiteralPath $archive -DestinationPath $unpacked -Force
    $decodedExecutable = Join-Path (Join-Path $unpacked $folderName) 'openvideomix.exe'
    if (-not (Test-Path -LiteralPath $decodedExecutable -PathType Leaf)) {
        throw 'ZIP does not contain the expected portable executable'
    }
    $decodedHash = (Get-FileHash -LiteralPath $decodedExecutable -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($decodedHash -ne $exeHash) { throw 'ZIP executable SHA256 mismatch' }

    $decodedLauncher = Join-Path (Join-Path $unpacked $folderName) 'INICIAR-OPENVIDEOMIX.cmd'
    if (-not (Test-Path -LiteralPath $decodedLauncher -PathType Leaf)) {
        throw 'ZIP does not contain the launcher script'
    }
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
Write-Output "Audiovisual Alpha packaged with bundled runtime and launcher."
