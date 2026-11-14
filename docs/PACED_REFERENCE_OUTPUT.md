# Paced reference Program output

This increment adds a rational deadline scheduler in `core::output_schedule`
and an optional `output_worker::OutputWorker` in the desktop library. It is a
CPU reference implementation, independent of egui. It does not establish sustained
1080p50 hardware performance, implement a GPU compositor or capture/mix audio.

## Timing contract

`OutputSchedule` derives every deadline from the absolute frame index using the
existing exact rational timebase. It never accumulates rounded frame periods.
When the worker is late, it renders only the latest due tick, counts missed
deadlines and keeps the original schedule phase. It never burst-renders backlog.
Backward clock readings, invalid rates and timestamp overflow are errors.

The worker reads the same GstSystemClock epoch as `CameraFrame.capture_pts_ns`.
Every packet carries the intended frame index, common-clock presentation timestamp,
skipped deadlines and scheduling lateness. Lateness is measured before CPU rendering;
it is not encode latency or end-to-end latency. No GUI refresh is used as a clock.

## Input and output policy

Callers explicitly supply a production profile, registered source IDs, requested
source rates, a fixed Program/overlay route, maximum hold time, and queue capacities.
Use the shared `VideoProfile::default()` for 1920x1080 progressive at 50/1. Source
requested rates are configuration, not measured or hardware-negotiated evidence.
Different requested input/output rates are rejected under `Exact`; selecting
`HoldLatest` explicitly permits reference frame selection/holding. Scaling and
interpolation are not implemented. Scene/route changes and desktop commands remain
on the existing shared command architecture; this worker only has a startup route.

Ingress and output channels are bounded. `try_submit` returns full/disconnected
errors with the owned frame, leaving retry/drop decisions to the caller. Ingress
is processed in bounded batches so busy producers cannot starve output deadlines
or cancellation. Rejected ingress is counted and the latest typed error retained.

Output delivery is nonblocking: if the consumer queue is full, the newly generated
packet is dropped and counted. `rendered_frames` counts composed frames, including
frames whose delivery was dropped; it is not a delivered or encoded-frame count.
Output queue drops count packets, including source-error packets. Missing frames
and stale sources generate explicit error packets and remain visible in stats
even if those packets cannot be delivered. Maximum hold is checked against
the **selected frame PTS**, so pending future frames cannot hide a stale held frame.

`stop` sets cancellation, wakes a sleeping worker and joins it without waiting for
queue consumers or a distant deadline. A CPU composition already in progress must
finish. Dropping the worker calls stop. Terminal clock/schedule errors remain in
the shared stats even when the output channel is full.

## Recording boundary and limits

Packets retain real timeline gaps. Use ProgramRecorder's explicit `write_frame_at`
API with rebased tick indices and `required_audio_samples_for_frame`, supplying
actual PCM for the complete elapsed interval. The contiguous `write_frame` API
must not be used to erase skipped/dropped worker slots. Choosing the recording
origin, matching the output cadence and providing synchronized captured/mixed
PCM still require integration before desktop RECORD can be enabled. There is no
recording thread, encoder, audio callback, real-time priority or GPU implementation
in this worker.
Stats are counters/snapshots, not a qualified broadcast-performance metric.

## Tests and continuation

Deterministic scheduler tests cover exact 50/1 and 60000/1001, missed-deadline
skipping without phase drift, no early/duplicate ticks, clock rewind and overflow.
Threaded tests use tiny synthetic frames and broad timeouts, checking timestamped
output, staleness despite a pending future frame, explicit cadence conversion,
invalid routes/capacities, observable input rejection, full output queues and fast
stop from a one-hour wait. An actual synthetic GStreamer session also reaches the
worker in the same clock epoch with exact reference pixels, using an explicit
640x360/30 experimental profile. They do not assert actual 50fps wall-clock performance.
The existing combined-feature Linux CI test job includes these tests.

Next: preserve packet timing gaps at the encoder boundary and supply synchronized
real audio, then integrate the production GPU Program tap. Qualify optional media
features on Windows/macOS and physical hardware. Merge #35 and its dependencies
before retargeting this branch to main and revalidating all required CI.
