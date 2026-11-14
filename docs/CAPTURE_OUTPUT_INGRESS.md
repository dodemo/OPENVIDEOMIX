# Capture-to-Program ingress (reference implementation)

`CaptureOutput` is a headless input boundary for an output worker. It owns the
existing timestamp queues and CPU reference compositor; it does not use egui,
start a production clock, or replace the GPU/audio work still required.

- Default configuration remains the shared 1920x1080 progressive 50/1 profile.
  An alternate profile must be passed explicitly. Dimensions must match; there
  is no implicit scaling. Source mode negotiation and measured capture FPS are
  still separate responsibilities. Buffer duration is not treated as measured FPS.
- Only `CameraFrame.capture_pts_ns` enters the playout queue. Missing common-clock
  timestamps are errors, even when a local `pts_ns` exists. Local PTS is preserved
  as metadata. Deadlines must use the same shared capture epoch, not GUI time.
- Duplicate registration is rejected without erasing queued frames. Invalid
  pixels, dimension mismatches, late/nonmonotonic timestamps, and missing sources
  produce typed errors. Existing frames remain usable after invalid ingress.
- Queues are bounded. Statistics distinguish accepted/rejected ingress frames
  and actual queue-overflow evictions. These are not encoder drops, measured FPS,
  source transport losses, or the number of frames skipped at output deadlines.
- At each caller-supplied deadline the reference output selects the latest valid
  frame, holds it when necessary, and applies transitions/OV1-OV4. It does not
  schedule real-time output or negotiate native source cadence. The caller must
  explicitly configure any rate conversion before production use.

Capture now lives in the desktop library and the GUI imports that implementation,
so headless consumers and the GUI share the same session type and clock epoch.

## Validation

`tests/capture_output.rs` covers delayed sources with identical local PTS but
separate common PTS, future-frame exclusion, holds, queue overflow accounting,
invalid input without queue loss, duplicate registration, removal, defaults and
explicit alternative dimensions. A real `videotestsrc` session also reaches
Program without any GUI.

With both optional features enabled, a test captures three frames, composes the
actual Program frame, submits that same frame to ProgramRecorder with supplied
PCM, finalizes WebM and decodes all three video frames. It explicitly selects the
existing experimental **640x360/30** source/output profile and generates PCM;
this is not physical camera/microphone validation or a 1080p50 performance claim.
The existing full-HD recorder round-trip regression remains enabled unchanged.

```sh
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Linux GStreamer CI now runs the combined-feature tests, in addition to the existing
separate feature checks. Windows/macOS optional GStreamer runtime qualification,
physical hardware, native camera selection, production GPU rendering, audio
capture/mixing/drift correction and a paced output worker remain pending.

Dependencies: #32 (recording, #30/#28 and #31) and #34 (capture timeline, #25/#23).
Integrate both chains before retargeting this branch to main and revalidating CI.
