# Headless Program recording proof of concept

This opt-in adapter records **supplied composited Program RGBA and supplied PCM**
to a playable WebM file. It runs independently of egui. It is an incremental
implementation for #8, not completion of production recording or the media engine.

## Signal and format

- `ProgramRecorder` in the desktop library accepts packed straight-alpha RGBA8
  frames, paired with interleaved stereo F32 PCM at 48,000 Hz.
- The default uses the shared `VideoProfile`: 1920×1080 progressive, **50/1 FPS**.
  A different profile must be passed explicitly. Dimensions and PCM sample counts
  are checked; no scaling, resampling or silent frame-rate replacement occurs.
- Frame PTS and duration use the rational timebase. Audio sample boundaries use
  integer cumulative sample indices, including fractional video rates. At 50/1,
  each video frame has 960 PCM sample frames per channel.
- Encoding is explicitly **CPU VP8 + Vorbis in WebM**. Startup checks plugin
  availability; GStreamer reports caps/encoder/I/O failures through the bus.
  Hardware encoding and codec fallback negotiation are not implemented.
- The encoded-buffer counter is attached to the VP8 encoder's output pad. It is
  separate from submitted video frames and submitted audio sample counts.
  Finalization duration measures drain time, not end-to-end audiovisual latency.

## Run the deterministic example

Install GStreamer runtime/development libraries and plugins providing `appsrc`,
`videoconvert`, `vp8enc`, `audioconvert`, `vorbisenc`, `webmmux`, and `filesink`.
The decoding tests also require `matroskademux`, `vp8dec`, and `vorbisdec`.

```sh
cargo run -p openvideomix-desktop --features gstreamer-recording \
  --example record_program -- program.webm
ffprobe -v error -show_streams -show_format program.webm
```

The example composes the existing CPU reference output: a red-to-blue MIX and
an alpha lower third on OV1, with a generated 1 kHz stereo tone. Its 50 encoded
frames and 48,000 sample frames represent one second on the media timeline.
Wall-clock generation may be slower; this is **not measured real-time 1080p50**.
No camera, microphone, display capture, or GUI screenshot is used.

## Explicit timeline gaps

`write_frame_at(index, frame, pcm)` records at an explicit rational frame index.
The first accepted index must be zero; later indices must increase and may skip.
`required_audio_samples_for_frame(index)` reports the exact number of supplied
PCM sample frames per channel needed since the previous accepted frame, including
all skipped video slots. The audio buffer starts where the previous accepted
buffer ended. No silence or duplicate video frame is generated. `write_frame`
remains the contiguous convenience API and continues after the last explicit index.

For example, indices 0, 1, 5, 6 at 50/1 produce video PTS 0, 20, 100, 120 ms.
The supplied PCM covers 140 ms in total (6,720 sample frames per channel at 48 kHz).
Recording statistics distinguish four submitted/encoded frames, seven timeline
slots, three omitted video slots and a 140 ms media duration. These omissions
are supplied timeline gaps, not measured encoder drops. The player may hold the
previous picture across a gap; the recorder does not encode replacement frames.

Missing/short PCM, repeated/backward indices, nonzero initial indices, overflow,
and PCM intervals larger than the bounded one-second transaction are rejected
before advancing indices or submitting buffers. The caller must stop/report an
unsupported large gap instead of relabeling frames or shortening the timeline.
Chunked independent audio submission is not implemented yet.

For worker packets, explicitly choose the first accepted tick as the recording
origin and rebase subsequent tick indices; supply actual PCM aligned to that origin
and covering every interval, including video gaps. The configured video cadence
must match. Real audio capture, buffering and clock/drift reconciliation are still
required; this API does not create those components or prove A/V hardware sync.

An independent offline example deliberately omits video slots 10–19 but supplies
continuous generated audio:

```sh
cargo run -p openvideomix-desktop --features gstreamer-recording \
  --example record_program -- program-gapped.webm --gapped
ffprobe -v error -count_frames -show_streams -show_format program-gapped.webm
```

It retains a one-second timeline and 48,000 PCM sample frames per channel with
40 encoded video frames, ten omitted slots and a **nominal** 50/1 profile. It does
not demonstrate measured 50 encoded FPS. WebM timestamps have muxer timecode
quantization; the tests use exact 20 ms boundaries for 50/1 and integer sample
accumulation separately for 60000/1001. Vorbis end padding is checked explicitly.

## Lifecycle and failure behavior

Call from an output worker. `start` reserves a new path without truncating existing
files. `write_frame` rejects bad dimensions/buffers/PCM before advancing indices.
Caller validation errors are recoverable. Downstream errors or full bounded
queues terminate the session instead of silently dropping A/V or accumulating
unbounded memory. Appsrc and pipeline queues are bounded. This adapter reports
encoder overload; it does not yet adapt bitrate, pace a production clock, or
implement a selectable drop policy.

`finish(timeout)` sends EOS on both streams, waits for muxer EOS or an error, and
shuts down. Empty sessions and timeouts fail. Dropping any session shuts down its
pipeline. Failed/aborted sessions may leave incomplete files at the reserved
path; these are not reported as successful recordings and are not auto-deleted.

## Validation and remaining integration

The integration test writes and decodes ten full-HD Program frames, validates
50/1 caps, 20 ms PTS spacing, the first/last composite pixels, and 48 kHz stereo
non-silent decoded PCM including peak/RMS and sample-count bounds. Vorbis is
lossy and may pad the final codec block; this is accounted for explicitly.
Other tests cover existing-file preservation, invalid profiles and input,
empty recordings, exact 60000/1001 audio sample accumulation, and fatal bounded
queue backpressure. CI adds the feature round trip and all-feature lint/build
checks to the existing Linux GStreamer job.

Still required: GPU Program tap; captured/mixed audio and drift correction;
source-to-master-clock mapping; live output worker and desktop start/stop/status;
hardware encoder negotiation; sustained latency/drop measurements; disk-full
and long-run validation; RTMP/SRT; packaging and plugin/codec distribution license
audit. The API uses portable GStreamer elements; feature compilation/runtime
on Windows and macOS Intel/Apple Silicon and physical hardware remain unverified.
The base workspace retains all four CI platform targets. Do not enable a GUI
RECORD button or close #8 on the strength of this proof of concept.
