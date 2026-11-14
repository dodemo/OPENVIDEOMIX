# Desktop Program test — functional CPU slice, not production qualification

## Study and objective acceptance

Public vMix references reviewed on 2026-11-14:
[Cut](https://www.vmix.com/help29/Cut.html) swaps Preview and Output immediately;
[Recording](https://www.vmix.com/help29/Recording.html) records Output. These
observable behaviors define this increment: independent test inputs in Preview,
one CUT command swaps routing, the actual composed Program is monitored and the
same signal is encoded into a playable audiovisual file. No proprietary code,
branding, screenshots or assets were incorporated.

The required clean-room Study → Build → Check → Refine method was evaluated from
[Craft Anything](https://github.com/storytold/craft-anything). Its README and root
manifest describe CLI/MCP inspection tools and a Rust application (observed
version 0.6.1, Rust 1.97, egui 0.36). The actual LICENSE-MIT and LICENSE-APACHE
texts were read: the repository offers MIT OR Apache-2.0. No Craft Anything code
or dependencies were incorporated; this is not an audit of a shipped package.
Installing its application was unnecessary for this increment's executable checks.

## Build and run

Install Rust and the platform's GStreamer development/runtime dependencies,
including videotestsrc, videoconvert, vp8enc, vorbisenc and webmmux. Then run:

```sh
cargo run --release -p openvideomix-desktop --features gstreamer-capture,gstreamer-recording
```

1. Click **Iniciar estudo de proba · 4 fontes 1080p50**. Four independent GStreamer
   sources request the shared progressive 1920×1080, 50/1 production profile:
   SMPTE bars with moving noise, red, blue and checkerboard. They are test signals.
2. Select **PVW T1–T4**, then **CUT**. Preview/Program swap through the existing
   typed dispatcher. The Program monitor displays the worker's actual composition.
   Space/Enter CUT shortcuts do not fire while editing a destination. No T-Bar or
   TAKE is shown; desktop MIX/WIPE remain unavailable.
3. Choose **GRAVAR TESTE** and explicitly start with a destination WebM path.
   The approved directory is created if needed. Existing files are refused.
   Audio is explicitly a generated 1 kHz, stereo 48 kHz test tone, affected by
   master volume/mute. It is not microphone/camera audio or a completed mixer.
4. Change Program with CUT, stop recording, and wait for Saved or an error.
   File creation/request acceptance alone is never a successful recording.
5. Independently verify the file (Python standard library plus FFmpeg/FFprobe):

```sh
python scripts/verify_program_recording.py path/to/program.webm
```

The verifier checks full decode, requested dimensions/rate/audio caps, increasing
20 ms-grid video PTS, a common origin and decoded audio duration within 100 ms of
video end (codec padding allowed). It reports effective packet rate, missing slots,
maximum gap and PCM peak/RMS separately. A nominal 50/1 header is not measured
50 fps delivery. This verifier targets the default VP8/Vorbis recording profile,
not arbitrary files with reordered video frames or alternate production profiles.

## Signal path and bounded failure behavior

Capture feeds the output worker directly; GUI subscriber polling cannot throttle
it. The desktop uses a 100 ms input lookback with 12-frame per-source history and
500 ms maximum source hold. Original and mapped capture PTS are retained. This
is a buffering choice, not exposure alignment or clock-drift correction.

The monitor and recording have separate consumers. Program is published even if
the two-packet recording delivery queue fills. Recording drops are counted; the
recorder retains actual output indices and supplies generated PCM for the whole
elapsed interval. It never compresses gaps into contiguous video. The encoder
consumer may wait at most one second for bounded encoder capacity. A gap beyond
the one-second PCM transaction or downstream failure aborts/finalizes the partial
file with an error; it is not reported Saved. EOS finalization is bounded at ten
seconds. GUI monitoring downsamples to at most 640×360; composition and recording
remain full 1920×1080. No GUI screenshot is encoded.

Other legacy GUI sources are not registered in this production graph. Routing one
of them into Program produces an explicit unavailable-source result; PNG overlays,
physical cameras and file inputs must be connected before extending this slice.

## Actual Check / Refine evidence

Native Linux app under Xvfb, software OpenGL, four-CPU quota, Rust 1.100.0,
GStreamer 1.26.2, release build. Code: `67f69cf` (later main integration adds the
camera-identity module without changing this signal path). No physical camera,
microphone, Windows PC or Mac was used. There was no exposed interactive desktop
viewer, so native mouse/keyboard events and window screenshots were captured on
the attached execution environment.

Refinement was driven by observed failures: egui layout-pass replay initially
applied CUT twice (now deduplicated per input event); full-HD GUI texture uploads
competed with production (now monitoring is downsampled); a slow encoder delayed
Program monitoring (now independent bounded delivery); expensive synthetic ball
capture lagged its own PTS (production patterns changed, without rewriting PTS).
Transient missing Program frames produce timestamped gaps; extended loss is an
error. Encoder saturation remains visible and unresolved as a performance gate.

Retained [evidence](evidence/desktop-program-2026-11-14/):

- [Recording screenshot](evidence/desktop-program-2026-11-14/recording.png).
- [Saved screenshot](evidence/desktop-program-2026-11-14/saved.png).
- [Actual native Program recording](evidence/desktop-program-2026-11-14/program.webm).
- [Independent file verification](evidence/desktop-program-2026-11-14/file-verification.json).
- [Decoded CUT pixel check](evidence/desktop-program-2026-11-14/cut-verification.json).

The 12.46-second recording decodes 214 VP8 frames at 1920×1080, nominal 50/1;
**effective file rate 17.17 frames/s**, 409 omitted slots, maximum video PTS gap
200 ms. Vorbis decodes 598,592 stereo sample frames at 48 kHz (12.4707 seconds,
10.7 ms codec padding), RMS −10.91 dBFS. Mean decoded pixels change from SMPTE
bars to red at decoded frame 81; the last 133 decoded frames are red. Saved UI
counts agree with file analysis. Worker delivery to the monitor reported zero
queue drops; that does not establish zero scheduling/source misses or displayed
50 fps. This is a short functional check, not a controlled performance comparison
against earlier debug/release attempts or a sustained endurance qualification.

Recording SHA-256:
`8bb39cb080938a18d56d127f937f6e7e545298009fe60835765678e54ebe0407`.

Automated coverage includes actual four-source capture with GUI queues deliberately
undrained, dispatcher CUT, full-HD Program recording/finalization/decoding and
changed recorded pixels/PCM. A blocked encoder delivery regression proves that
new Program frames continue to replace the monitor image despite saturation.
Other regressions cover GUI command replay, bounded monitoring, explicit recording
intents, gap PTS and audio sample alignment. See the current checkpoint for counts.

## Ready boundary and next work

The **test-video → Preview → CUT → actual CPU Program → playable test-tone
recording** path is functional. It is not first-release acceptance. Required next:
measured GPU Program composition and encoder negotiation, live independently
buffered audio with clock/drift management, physical source/mode selection,
other source types, transitions/overlays/Stinger, streaming, control feedback,
installers and issue #33 license/security/distribution gates. The owner's RTX
4080 SUPER Windows bench remains untested; use [the physical procedure](WINDOWS_HARDWARE_TEST.md).
