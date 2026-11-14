# Windows physical validation bench — procedure, not executed evidence

Owner-provided target: Windows 10 Pro, RTX 4080 SUPER 16 GB, 96 GB RAM, 2 TB NVMe.
This computer has not been remotely accessed or tested by Codex. It does not
qualify Windows 11, Linux, macOS Intel or Apple Silicon. Do not mark release gates
complete from these instructions. Record exact commit, OS/driver/runtime versions,
connected device models, mode/caps and test duration in the actual results.

## Prepare and record versions

Install the supported Rust toolchain and GStreamer MSVC runtime/development
packages, including the plugins needed by the enabled features. Make GStreamer's
bin and pkg-config metadata available. Install FFmpeg/FFprobe and Python for file
verification. Check their actual presence rather than assuming an installer did it.

```powershell
git rev-parse HEAD
rustc --version
cargo --version
gst-inspect-1.0 --version
ffmpeg -version
nvidia-smi --query-gpu=name,driver_version,memory.total --format=csv
Get-ComputerInfo | Select-Object WindowsProductName,WindowsVersion,OsBuildNumber
```

Do not publish hostnames, serial numbers, local account names, tokens or stream
keys. Check `gst-inspect-1.0 vp8enc`, `vorbisenc`, `webmmux` and `videotestsrc`.
Checking `nvh264enc` only discovers an encoder plugin; this revision's recorder
uses CPU VP8, so RTX presence does **not** establish NVENC recording support.

## Current functional desktop slice

```powershell
cargo run --release -p openvideomix-desktop --features gstreamer-capture,gstreamer-recording
```

1. Start the explicit four-source 1080p50 test studio. Check motion/source identity
   in Preview. These are GStreamer test signals, not physical cameras.
2. Select a test source, then CUT. Preview and Program swap, with the actual
   composed Program signal appearing in the Program monitor. Repeat with the
   other sources. The final UI has no T-Bar/TAKE; MIX/WIPE remain unavailable here.
3. Choose GRAVAR TESTE, inspect/approve the destination, and start. Audio is an
   explicit generated 1 kHz stereo 48 kHz tone; microphone audio is not captured.
   Check recording state, counter and real file creation. Existing files must
   be refused. Change Program with CUT while recording. Master volume/mute
   affects the supplied test PCM; other audio graph functions remain unfinished.
4. Stop and wait for a successful saved-file result or an explicit error. Inspect
   actual encoded count and omitted slots. Decode independently:

```powershell
python scripts/verify_program_recording.py C:\path\to\program.webm
```

The tool reports nominal profile separately from packet count/duration and PTS
gaps, plus actual decoded PCM levels. This is file validation, not proof that a
GPU/audio/capture pipeline sustained broadcast cadence. Capture a 60-second and
then a 30-minute run, retaining worker missed deadlines/queue drops, file evidence,
CPU/GPU/memory and disk behavior. Compare debug and release builds separately.
Use Task Manager and `nvidia-smi` to record observations; never infer GPU composition
or hardware encoding from the mere presence of a GPU/driver.

## Release gates still requiring implementation and physical tests

Physical UVC/USB/HDMI cameras must be individually selected, their actual modes
reported and 1080p50 negotiated without hidden conversion. Native microphone/UAC
audio, synchronization/drift, flash/clap A/V latency, device removal/reconnection,
GPU Program composition, NVENC negotiation, RTMP failure/recovery and installer
clean-install/signing checks are not qualified by the test studio. Record each
unsupported operation as incomplete, not passed. Keep screenshots/output files
and precise reproduction steps for failures, and redact private details before
uploading reports to public GitHub.
