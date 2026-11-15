# Windows x64 portable GUI demonstration (issue #46, first bounded slice)

This is an **intermediate CI deliverable**, not the Windows audiovisual Alpha.
It makes a native Windows x64 `openvideomix.exe` available as a downloadable
ZIP without installing Rust, Python or development tools on the destination PC.

## Build and download

1. Open the GitHub Actions workflow **Windows portable demo artifact** for this
   branch / pull request. Wait for a green `Portable Windows x64 demo (no media)` job.
2. Download its **OPENVIDEOMIX-Windows-Demo-<commit>** artifact. GitHub may
   wrap the uploaded ZIP and its checksum file inside another artifact ZIP.
3. Verify the SHA256 of the inner `OPENVIDEOMIX-Windows-Demo-...zip` with the
   adjacent `.sha256` file, extract it, and double-click `openvideomix.exe`.
4. Inspect `build-manifest.json`, which identifies the exact source commit,
   version, build features, executable hash and tests **not yet performed**.

This ZIP contains the program, the project MIT LICENSE, the manifest and a
Galician README. It is built on GitHub-hosted `windows-2022` (Windows Server),
not on the owner's physical Windows 11 workstation. The script validates a
Windows x64 PE header and the ZIP's executable byte-for-byte checksum.

The CI job has a **35-minute limit**, only runs automatically when its own
workflow, packaging script or this document changes, and uploads artifacts with
**7-day retention**. No local coding agent, Ollama, paid AI service, personal
token or continuously running PC is needed.

## Strict limits of this package

- The release has **default Cargo features only**: no GStreamer runtime, no
  real device capture, no working Program recording, no live microphone and no
  broadcast streaming. UI-only demo elements are not real inputs.
- 1920x1080 progressive at 50/1 FPS and 48 kHz remain the design defaults,
  **not measured media output** from this build.
- A Windows-hosted compilation is **not** proof of clean, disconnected Windows 11
  launch. This requires a separate real Windows 11 PC/VM test and exact report.
  Static CRT linking is requested, but native import/dependency inventory and
  no-network cold launch still need verification.
- This package cannot satisfy the offline-install acceptance gates of #46.
  Do not advertise it as a production-ready release or a complete Alpha.

## Next build and acceptance gates

1. Compile the desktop with `gstreamer-capture,gstreamer-recording` on a
   Windows MSVC runner using **official GStreamer runtime and development
   binaries**. Do not claim this until the feature build and native tests pass.
2. Package only approved, license-inventoried plugins and their full transitive
   native DLL dependencies, private plugin scanner and runtime data. In
   particular, do not rely on a global GStreamer installation or PATH on the
   destination PC. Audit LGPL/GPL and codec redistribution requirements.
3. Test the resulting ZIP on a clean Windows 11 machine **without internet or
   development tools**. Start four synthetic sources, Preview/CUT/Program,
   record WebM and independently decode the file, measuring actual PTS/FPS,
   gap/drops, 48 kHz PCM and any startup failures.
4. Only then test real physical camera and microphone negotiation, with owner
   permission. Hardware capture, long-duration 1080p50, measured A/V
   synchronization and NVENC remain separate checks.

These are open tasks, not claims of completed capabilities. See
[Windows hardware acceptance](WINDOWS_HARDWARE_TEST.md) and
[desktop Program test](DESKTOP_PROGRAM_TEST.md).
