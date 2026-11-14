# Development continuation checkpoint — 2026-11-14

## Latest continuation: desktop actual Program and test-tone recording

Active source branch: `feat/desktop-program-output`.
Functional source commit: `67f69cf`.
Latest tested code: `d58dc08` (normal merge of current main; adds camera identity).
This checkpoint/evidence update is a subsequent documentation commit.

**First release remains incomplete.** The synthetic four-input desktop path now
works end to end: actual GStreamer 1080p50 test sources → Preview → typed CUT →
actual CPU Program → timed VP8/Vorbis WebM with explicit generated 48 kHz stereo
PCM. Preview and Program swap. Same full-resolution Program frame is monitored
and supplied to recording; only GUI monitoring is downsampled. No T-Bar/TAKE.
MIX/WIPE and RTMP remain disabled. Legacy GUI/physical/image/file sources are not
yet registered in this production graph. No captured microphone audio is implied.

Capture/Program monitoring and recording run independently of GUI subscriber
polling and of encoder backpressure. Recording delivery is bounded to two packets;
drops are counted and output frame indices preserve video gaps. A complex encoder
frame may wait up to one second off the GUI; beyond bounded PCM/encoder limits,
finalize a partial file with an error. Directory approval and no-overwrite are
required; ephemeral recording intents are never replayed from saved state.
Observed egui repeated-pass double CUT is fixed with per-input deduplication.
Source lookback is explicit (100 ms), not a timestamp rewrite or physical sync.

### Verified evidence

Linux only, Rust 1.100.0, GStreamer 1.26.2, four-CPU quota, Xvfb/software OpenGL.
Format, all-feature workspace check, all-target/all-feature Clippy and all-feature
workspace tests pass at d58dc08: **99 tests** (64 core + 10 library + 5 GUI binary +
5 ingress + 1 desktop media service + 6 worker + 8 recording). Default workspace,
capture-only and recording-only compilation checks also pass. Manifest metadata
warnings from the new Cargo version remain distinct from compiler/Clippy failures.

The actual release application was driven with native mouse/keyboard events:
start four sources, CUT SMPTE to Program, select red in Preview, start recording,
CUT to red, stop/save, quit cleanly. Screenshots and actual WebM are retained in
`docs/evidence/desktop-program-2026-11-14/`. Independent FFprobe/FFmpeg verification:
12.46 seconds, 214 decoded VP8 frames, 1920×1080, nominal 50/1, **17.17 effective
frames/s**, 409 missing slots, max gap 200 ms; Vorbis 48 kHz stereo, 598,592 decoded
sample frames, 10.7 ms padding, RMS −10.91 dBFS. Decoded pixels switch from bars to
red at frame 81, with 133 red frames. The Program monitor delivery queue had zero
drops; scheduler/source misses are separate. Encoder saturation is unresolved.
No real-time 1080p50, hardware GPU, captured A/V sync, physical device, Windows or
macOS runtime claim. No physical owner workstation access occurred.

See `docs/DESKTOP_PROGRAM_TEST.md` for the mandatory public-reference Study → Build
→ Check → Refine evidence and exact run instructions, plus
`docs/WINDOWS_HARDWARE_TEST.md` for the future RTX 4080 SUPER bench procedure.
`python scripts/verify_program_recording.py docs/evidence/desktop-program-2026-11-14/program.webm`
repeats the retained file verification. Craft Anything's README, manifest and both
actual root licenses were read; only its method was applied, no code incorporated.

### Live integration state (re-query exact heads before mutations)

Merged with all available exact-head CI jobs passing and empty review threads:
#28 rational timing (c32fe59), #27 camera identity (ff9c032), #23 concurrent capture
(779e818), #30 atomic reference output (72777d4), #31 default profile (90dd140).
Current inspected main: `90dd1409f561b61fb3c8d4c1c210652f2343df62`.
No issue was closed. Rulesets API returned an empty list; branch protection remains
HTTP 403 to this integration. No safeguards changed, no forced history rewrite.

- #25: `20e2829`, base main, normally integrated main including #31. Local 59 core
  tests and format pass; fresh integrated CI pending at checkpoint creation.
- #32: `a831133`, retargeted main after #30/#31. Normal main merge resolves only
  the camera_registry export conflict. Local 59 core tests/Clippy and format pass;
  fresh integrated media/platform CI pending. Reintegrate/revalidate after #25.
- #34: e8452ba, base #25. After #25 merges, normally integrate main, retarget,
  rerun full applicable checks and inspect review threads before merging.
- #35: 48dd39b, requires #32 and #34; then retarget/main integration/full CI.
- #36: b6ba79a, base #35; #37: fc6370b, base #36. Do not skip dependency gates.
- Desktop branch: stacked on #37; includes a normal main merge. Publish/inspect
  the new draft PR and keep it draft until its dependencies and required CI pass.

Another remote branch, `chore/codex-github-coordination-pilot`, appeared during
fetch. It was not modified or integrated. Preserve remote owner work.

### Exact next executable work

1. Inspect the new desktop PR checks at its exact pushed head; diagnose failures.
   Complete pending dependency integrations above with normal merges and full CI.
2. The desktop slice is functionally usable for test signals but recording is
   below broadcast cadence. Measure capture/composition/encoding separately and
   implement the real-time GPU Program path and explicit encoder negotiation,
   keeping the actual Program tap and CPU reference as correctness oracle. Do
   not hide this bottleneck by changing the default profile, inventing video
   frames or shrinking timestamps. The cloud has software graphics only; record
   adapter/backend identity and require physical GPU qualification later.
3. Implement independently buffered real 48 kHz audio capture/mix and common-clock
   mapping/drift management; then replace test tone only through an explicit
   operator choice. Add decoded output tests and observable synchronization tests.
4. Connect persistent physical camera identity/mode selection and other real
   sources, then transitions/overlays/Stinger, streaming and control feedback.
   Issue #33 lockfiles/security/license/SBOM/installers remain open release gates.

A single enabled thread-attached recurring continuation exists. This interactive
run owns `refs/heads/codex/continuation-lock` at c32fe5994e175f6df823570d9a9294db8fcde3a6;
scheduled messages arriving while it is active do not create a second writer.
At a clean handoff, push the latest complete checkpoint, update the automation to
this branch, verify ownership and delete only this ephemeral ref. Never steal or
expire a stale lock. The old separate automation remains paused. No subagents or
external paid services were created. Future environment inheritance is not proof
that a future run has executed successfully.


## Latest continuation: paced reference output worker (#36)

Current branch: `feat/paced-reference-output`.
Tested code: `dadf080cccd27c6ddbcb736455da08ffcf975ff5`.
This checkpoint update is a subsequent documentation-only commit.

Implemented exact rational `OutputSchedule` and a bounded, nonblocking CPU
`OutputWorker` owning CaptureOutput on the shared capture-clock epoch. Late work
skips/counts missed deadlines without phase drift. Packets preserve intended PTS,
frame index, skipped deadlines and scheduling lateness. Explicit HoldLatest is
required when configured source/output rates differ. Source staleness uses the
selected frame PTS, so pending future frames cannot hide a stale image. Errors,
rejected input, output packet drops and terminal state are observable. Stop wakes
a sleeping worker and joins without consumer cooperation; a running CPU render
must finish. Routes are startup configuration, not a second command architecture.

Local Linux format, workspace check, all-feature/all-target Clippy and all-feature
workspace tests pass: **84 tests** (56 core + 9 library + 3 desktop + 5 ingress +
6 worker + 5 recording). Default workspace tests also pass. Four deterministic
scheduler tests cover 50/1, 60000/1001, exact boundaries, missed deadlines, rewind
and overflow. Worker tests cover queue overload, explicit conversion, bad routes,
source staleness despite future frames, rejected ingress, clean stop during a
one-hour wait and actual synthetic GStreamer capture feeding the shared-clock
worker. These tiny-frame tests do not qualify sustained real-time 1080p50.

Merge #35 and its dependency chains first, then retarget #36 to main and rerun
required CI. No history rewritten; no PR merged or issue closed in this continuation.
All previously inspected open PRs still had pending CI, with no failed completed
conclusions observed. Re-query current heads/jobs before acting. In particular,
macOS jobs continued to block otherwise passing dependencies.

Exact next task: add an explicit timestamp/gap-aware recording API. The current
ProgramRecorder assigns contiguous indices, while OutputWorker may skip deadlines
or drop delivery packets. Preserve the output tick's relative video PTS and require
real supplied PCM covering the corresponding audio interval; do not compress time,
invent silence or silently reinterpret dropped packets as contiguous video. Add
playable-file regressions with gaps and 48 kHz sample alignment. Integrate real
capture/mixed audio and the production GPU Program tap before desktop RECORD.
See `docs/PACED_REFERENCE_OUTPUT.md` for implementation limits and lifecycle.


## Previous continuation: capture-to-Program ingress (#35)

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
