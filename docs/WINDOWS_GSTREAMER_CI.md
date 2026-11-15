# Native Windows GStreamer media CI (issue #46)

This is an **MSVC build and synthetic recording gate**, not a consumer package.

The [Windows GStreamer media smoke workflow](../.github/workflows/windows-gstreamer-smoke.yml)
runs only when the workflow or its PowerShell test changes, or when manually
dispatched. It runs on a GitHub-hosted Windows Server 2022 machine, with a
**45-minute timeout**, read-only GitHub permissions and cancelled superseded
attempts. It does not launch local LLM agents or use the owner's workstation.

## Upstream and security

The runner downloads the **official GStreamer 1.28.8 MSVC x86_64** Inno Setup
installer and its `sha256sum` from the same official TLS host
(`gstreamer.freedesktop.org`). It checks the SHA256 before execution and
installs **only on the ephemeral CI runner** in a dedicated folder with
`/CURRENTUSER /TYPE=devel`. A hash fetched from the same origin is a download
integrity check, not an independent signature verification or a complete
supply-chain audit. No downloaded installer, plugin, codec or device media
is republished by this workflow.

Upstream documentation:
- https://gstreamer.freedesktop.org/download/
- https://gstreamer.freedesktop.org/documentation/installing/on-windows.html
- https://gstreamer.freedesktop.org/documentation/deploying/windows.html

## Checks

- Verify that `pkg-config` reports the native GStreamer version, and
  synthetic input, VP8/Vorbis encoders, WebM mux/demux and decoders exist.
- Check the Rust desktop with `gstreamer-capture,gstreamer-recording`.
- Run the media-enabled Rust **library** tests.
- Run the independent `record_program` example with synthetic video and 48 kHz
  PCM, require a nonempty WebM, then inspect its audio and video streams using
  the separate GStreamer discoverer tool.

Failures fail the job. No fake green fallback or conditional skip is allowed.
The test is bounded by the workflow timeout.

## Not yet accepted

- This test does **not** prove a portable Windows installation or DLL closure,
  redistributable plugin licenses, executable GUI interactions, physical
  cameras/microphones, WASAPI device capture, a sustained 1080p50 cadence,
  real-time latency, AV drift or a clean offline Windows 11 run.
- The installer is **not bundled** into a release ZIP. Private GStreamer
  packaging needs a complete runtime/codec license inventory and acceptance
  before publishing any consumer-ready Windows Alpha.
- Windows Server 2022 CI is not the owner's Windows 11 PC with RTX 4080 SUPER.

Separate end-user deployment and physical testing remain acceptance gates in
[issue #46](https://github.com/dodemo/OPENVIDEOMIX/issues/46) and
[WINDOWS_HARDWARE_TEST.md](WINDOWS_HARDWARE_TEST.md).
