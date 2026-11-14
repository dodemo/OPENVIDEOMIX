# Development continuation checkpoint — 2026-11-14

## Latest continuation: capture-to-Program ingress (#35)

Current branch: `feat/capture-program-ingress`.
Tested code commit: `a0b9c7dc5c173dddc7c8a3f62ae88e7b4bf5338b`.
This update is a subsequent documentation-only commit on the same branch.

Implemented `CaptureOutput`, requiring common-clock capture timestamps for the
existing bounded ReferenceOutput queues. It preserves source-local PTS metadata,
rejects missing timestamps, malformed pixels, mismatched dimensions and duplicate
registration, and exposes accepted/rejected ingress and actual overflow counts.
Capture is exported from the desktop library and reused by the GUI, so headless
and GUI consumers share one implementation and clock epoch.

Local Linux validation at this code revision: format, workspace check,
all-target/all-feature Clippy and all-feature workspace tests pass: **74 tests**
(52 core + 9 library + 3 desktop + 5 capture ingress + 5 recording).
A new combined-feature regression captures three real synthetic GStreamer frames,
composes Program, records those same frames with supplied PCM, finalizes WebM and
decodes all three. The test explicitly selects experimental 640x360/30. The shared
1080p50 default and full-HD recorder regressions remain unchanged. Other ingress
tests cover delayed sources, future-frame exclusion, holding, rejected input
without queue loss, removals and overflow accounting. CI now runs combined-feature
tests, in addition to separate capture/recording feature tests.

Merge dependency: #35 includes both #32 and #34 via a normal merge. Complete the
chains below first, then retarget #35 to main and revalidate. No history rewrite,
no PR merged and no issue closed in this continuation. At the last CI query, all
nine open PRs (#23/#25/#27/#28/#30/#31/#32/#34/#35) had pending checks and no failed
completed conclusion. #27 and #28 each still awaited a macOS job. Do not treat
queued or running CI as success.

Exact next task: add a bounded, paced output worker owning CaptureOutput, with a
rational output schedule in the shared clock epoch, explicit source cadence/rate
conversion configuration and tests for missed deadlines, overload, clean stop and
source loss. Before activating desktop RECORD, implement the GPU Program tap and
real captured/mixed audio with A/V clock/drift management. Current integration is
CPU/reference and supplied PCM; it is not a production worker or measured 1080p50.
See `docs/CAPTURE_OUTPUT_INGRESS.md` for the boundary and limitations.

## Previous checkpoint (historical snapshot)

## Repository and current work

Repository: https://github.com/dodemo/OPENVIDEOMIX

Current branch: `feat/shared-capture-timeline` (PR #34).
Tested code commit: `1eed19cb7cd1647c03be1058d557ba255cabaafb`.
This checkpoint is a subsequent documentation-only commit on that branch.
Recording branch: `feat/headless-program-recording` (PR #32), code commit
`8a466fa2de05a29919ec5958d708ce3810197c0e`.

At inspection, `main` was `fa6395ba0105c3d0d0dde2cf0a11088e1abdc0e9`;
PR #22 was already merged before this session. No PR was merged and no issue
was closed during this session. No branch history was rewritten.

## Implemented and published

- #30, `b1bd0983fb49e1a518b6b9dc006c49a53a0813e1`: failed reference renders no
  longer consume source frames or advance playout cursors. Global output time
  cannot rewind when routing changes. Both bugs were reproduced by regression
  tests before the fix. Updated the existing PR description.
- #25, `4c4df7d23523dc0a9e08d3671994e32e78a8abbb`: merged the current #23 branch
  into its dependent discovery branch, preserving recent source PTS transport,
  full-queue error delivery, and synthetic 1080p50 tests. Updated the PR.
- #32: headless ProgramRecorder accepts supplied composed RGBA plus actual PCM
  data and encodes VP8/Vorbis WebM. Uses shared default 1080p50/48 kHz, bounded
  queues, explicit errors, encoder-side frame counts and EOS finalization.
  Includes a deterministic CPU Program MIX + lower-third + generated tone example,
  round-trip decoding tests, documentation and Linux all-feature CI gates.
- #34: shared capture clock/epoch, segment-aware mapping to `capture_pts_ns`,
  original local PTS retained, checked missing/invalid metadata behavior and
  RAII pipeline cleanup on startup/error exits. Two new synthetic/unit tests.
- #8 remains open, with an implementation-progress comment linking #32 and its
  remaining production limitations.

## Actual local validation

Linux only; Rust 1.100.0, GStreamer 1.26.2. No physical camera, audio interface,
MIDI, Stream Deck, OBSBOT, Windows host or Mac was available.

| Revision | Commands and observed results |
| --- | --- |
| #30 `b1bd098` | Format, workspace check, workspace tests and workspace Clippy passed; 45 core tests. Capture feature smoke tests and combined capture/MIDI/API lint also passed. |
| #25 `4c4df7d` | Format, workspace check, all-feature workspace tests (28 core + 9 desktop), all-feature/all-target Clippy passed. Includes concurrent synthetic sources and headless discovery. |
| #27 `b5f281c` | Format, 35 core tests, core all-target Clippy passed on an isolated review checkout. |
| #32 `8a466fa` | Format, workspace check, all-feature/all-target Clippy, all-feature workspace tests passed: 52 core + 1 recorder unit + 4 desktop + 5 recorder integration = 62 tests. |
| #34 `1eed19c` | Format, workspace check, all-feature/all-target Clippy, all-feature workspace tests passed: 28 core + 11 desktop = 39 tests. |

Full checks used `cargo fmt --all -- --check`, `cargo check --workspace`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, and
`cargo test --workspace --all-features` unless a narrower check is stated.
The new Cargo version emits pre-existing manifest unused-dependency warnings;
no compiler/Clippy errors remained in the successful final checks.

Independent #32 file validation: the example generated 50 encoded VP8 frames
at 1920x1080, `r_frame_rate=avg_frame_rate=50/1`, and Vorbis stereo 48000 Hz.
FFprobe reports a 1.000000-second WebM with both streams starting at zero.
`ffmpeg -v error -i program-smoke.webm -f null -` decoded it without errors.
Generation took **30.18 seconds in debug** for one second of media; this is an
offline reference result, not real-time 1080p50. Round-trip tests independently
verify decoded Program pixels, 20 ms video PTS spacing, audio samples/peak/RMS.

## Dependency and CI state — re-query before acting

All eight PRs below were open at this checkpoint, with incomplete platform CI.
No failing CI conclusion had been observed; pending jobs are not passes.
In particular #28 and #27 still had `Base / macos-15` pending; other branches
also had macOS Intel/Apple Silicon jobs pending. New #32/#34 jobs were running.
Do not disable gates, replace runner targets without evidence, or assume queued
jobs will pass. Do not merge Draft PRs merely because local Linux tests pass.

| PR | Current base / dependencies | Next action |
| --- | --- | --- |
| #23 | main; #22 already merged | Verify complete platform CI, review and merge when eligible. |
| #25 | #23 branch | After #23, retarget main, validate integrated CI, then merge. |
| #27 | main, independent identity model | Complete platform CI before merge; hardware adapter integration remains separate. |
| #28 | main, rational timebase | Complete platform CI before merge; then retarget #30. |
| #30 | #28 branch | After #28, retarget main and revalidate before merge. |
| #31 | main, shared profile | Complete platform CI before merge; does not prove device 1080p50 support. |
| #32 | #30 branch, also includes #31 by normal merge | Merge #28/#30/#31 first, retarget main and revalidate. |
| #34 | #25 branch | Merge #23/#25 first, retarget main and revalidate. |

Open issues reviewed: #2 (future scope), #3, #4, #5, #6, #7, #8, #10, #13, #14.
All remain open: none meets the whole requested acceptance criteria yet.

## Remaining blockers and exact next work

1. Re-query CI for the latest exact head SHA of each PR. Inspect failures if any;
   merge eligible dependencies in the order above only after every required gate.
   Read review threads and branch rules immediately before merging.
2. Independent implementation: connect shared capture PTS to an output-worker
   input boundary, **without** routing raw pipeline-local PTS into ReferenceOutput.
   Add a deterministic delayed-source test that feeds comparable timestamps into
   bounded queues and verifies Program selection at supplied output deadlines.
   Keep source metadata, requested mode and measured capture/output metrics distinct.
3. Establish the production GPU compositor/Program tap and real 48 kHz audio graph,
   including audio master clock/drift management, before wiring the recorder to
   desktop start/stop. Reuse ProgramRecorder's signal boundary; never screenshot
   the GUI or record a second approximation. Retain existing typed command routing.
4. Continue physical camera selection/capability negotiation and actual captured
   media, then recording/encoder negotiation and RTMP. Stinger remains mandatory
   for the first production release; no T-Bar/TAKE in the approved final UI.

GPU rendering, physical multi-camera 1080p50, captured/mixed audio, hardware
encoding, RTMP/SRT, Stinger, screen/file sources, full control feedback, packaging,
license audits, and physical cross-platform qualification remain incomplete.
The shared capture clock does not establish exposure alignment or drift correction.
The recorder feature's Windows/macOS compilation/runtime is not yet qualified;
base CI on those platforms does not qualify optional GStreamer media features.

All progress is committed/pushed to GitHub. No unattended continuation or future
execution is implied by this checkpoint.
