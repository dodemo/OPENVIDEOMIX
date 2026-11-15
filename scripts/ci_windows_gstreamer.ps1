# Hosted Windows-only CI preflight, never run on an end user's workstation.
# No GStreamer binaries or redistributable installers are uploaded by this job.
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Set-StrictMode -Version Latest

if (-not $env:RUNNER_TEMP) {
    throw 'This CI smoke requires a GitHub-hosted Windows runner (RUNNER_TEMP)'
}

$version = '1.28.8'
$fileName = "gstreamer-1.0-msvc-x86_64-$version.exe"
$origin = "https://gstreamer.freedesktop.org/data/pkg/windows/$version/msvc"
$installerUrl = "$origin/$fileName"
$digestUrl = "$origin/$fileName.sha256sum"
$installerPath = Join-Path $env:RUNNER_TEMP $fileName
$digestPath = Join-Path $env:RUNNER_TEMP "$fileName.sha256sum"
$installRoot = Join-Path $env:RUNNER_TEMP 'openvideomix-gstreamer-msvc'

Write-Output "Downloading GStreamer $version from official upstream (MSVC x86_64)"
Invoke-WebRequest -Uri $digestUrl -OutFile $digestPath -TimeoutSec 120
$sumFile = Get-Content -LiteralPath $digestPath -Raw
$match = [regex]::Match($sumFile, '(?i)\b[0-9a-f]{64}\b')
if (-not $match.Success) { throw 'Upstream checksum format is invalid' }
$expectedSha = $match.Value.ToLowerInvariant()
Invoke-WebRequest -Uri $installerUrl -OutFile $installerPath -TimeoutSec 1200
$actualSha = (Get-FileHash -LiteralPath $installerPath -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualSha -ne $expectedSha) { throw 'Official GStreamer installer SHA256 mismatch' }
Write-Output "PASS: official installer checksum $actualSha"

$setupArgs = @(
    '/VERYSILENT',
    '/SUPPRESSMSGBOXES',
    '/NORESTART',
    '/CURRENTUSER',
    '/TYPE=devel',
    "/DIR=$installRoot"
)
$setup = Start-Process -FilePath $installerPath -ArgumentList $setupArgs -Wait -PassThru
if ($setup.ExitCode -ne 0) {
    throw "GStreamer installer failed with exit code $($setup.ExitCode)"
}

$bin = Join-Path $installRoot 'bin'
$inspect = Join-Path $bin 'gst-inspect-1.0.exe'
$launch = Join-Path $bin 'gst-launch-1.0.exe'
$discover = Join-Path $bin 'gst-discoverer-1.0.exe'
$pkgConfig = Join-Path $bin 'pkg-config.exe'
if (-not (Test-Path -LiteralPath $pkgConfig)) {
    $pkgConfig = Join-Path $bin 'pkgconf.exe'
}
foreach ($required in @($inspect, $launch, $discover, $pkgConfig)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "Missing required GStreamer build/runtime tool: $required"
    }
}

$env:PATH = "$bin;$env:PATH"
$env:PKG_CONFIG = $pkgConfig
$env:PKG_CONFIG_PATH = (
    (Join-Path $installRoot 'lib\pkgconfig'),
    (Join-Path $installRoot 'lib\gstreamer-1.0\pkgconfig'),
    (Join-Path $installRoot 'share\pkgconfig')
) -join ';'
$env:GSTREAMER_1_0_ROOT_MSVC_X86_64 = $installRoot
$env:GST_PLUGIN_SYSTEM_PATH_1_0 = Join-Path $installRoot 'lib\gstreamer-1.0'
$env:GST_REGISTRY = Join-Path $env:RUNNER_TEMP 'openvideomix-gst-registry.bin'

$gstVersion = & $pkgConfig --modversion gstreamer-1.0
if ($LASTEXITCODE -ne 0) { throw 'GStreamer pkg-config package is missing' }
Write-Output "PASS: native pkg-config GStreamer version $gstVersion"
foreach ($element in @(
    'appsrc',
    'videotestsrc',
    'videoconvert',
    'vp8enc',
    'vorbisenc',
    'webmmux',
    'matroskademux',
    'vp8dec',
    'vorbisdec'
)) {
    & $inspect $element *> $null
    if ($LASTEXITCODE -ne 0) {
        throw "Missing runtime element: $element"
    }
}
Write-Output 'PASS: required synthetic encode/decode plugins present'

Push-Location -LiteralPath (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
try {
    & cargo check -p openvideomix-desktop --features gstreamer-capture,gstreamer-recording
    if ($LASTEXITCODE -ne 0) { throw 'Windows media-feature cargo check failed' }

    & cargo test -p openvideomix-desktop --features gstreamer-capture,gstreamer-recording --lib
    if ($LASTEXITCODE -ne 0) { throw 'Windows media-feature library tests failed' }

    # Synthetic CPU VP8/Vorbis recording: no camera, microphone or GPU required.
    $file = Join-Path $env:RUNNER_TEMP 'openvideomix-windows-synthetic.webm'
    & cargo run -p openvideomix-desktop --features gstreamer-recording --example record_program -- $file
    if ($LASTEXITCODE -ne 0) { throw 'Synthetic Windows WebM recording failed' }
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) {
        throw 'Synthetic recording produced no file'
    }
    if ((Get-Item -LiteralPath $file).Length -lt 1024) {
        throw 'Synthetic recording file is unexpectedly small'
    }

    $inspection = & $discover $file 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "GStreamer could not inspect WebM: $inspection" }
    if ($inspection -notmatch 'video' -or $inspection -notmatch 'audio') {
        throw 'Independent media inspection did not find both audio and video'
    }
    Write-Output 'PASS: Windows native GStreamer compiled and produced synthetic audiovisual WebM'
    Write-Output 'NOT QUALIFIED: portable/offline runtime packaging, real hardware, measured 1080p50, AV drift.'
}
finally {
    Pop-Location
}
