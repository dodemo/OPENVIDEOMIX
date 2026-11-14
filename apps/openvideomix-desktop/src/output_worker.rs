//! Bounded, paced CPU reference output worker. No GUI, audio or encoder thread.
use crate::{
    capture::{capture_clock_now_ns, CameraFrame},
    capture_output::{CaptureOutput, IngressError},
};
use openvideomix_core::{
    camera::FrameRate,
    output_schedule::{OutputSchedule, OutputTick},
    reference_compositor::RgbaFrame,
    video_profile::VideoProfile,
    InputId,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CadencePolicy {
    Exact,
    HoldLatest,
}
#[derive(Clone)]
pub struct WorkerSource {
    pub input: InputId,
    pub requested_rate: FrameRate,
    pub cadence: CadencePolicy,
    pub queue_capacity: usize,
}
pub struct WorkerConfig {
    pub profile: VideoProfile,
    pub sources: Vec<WorkerSource>,
    pub program: InputId,
    pub overlays: [Option<InputId>; 4],
    pub channel_capacity: usize,
    pub max_hold: Duration,
    pub start_delay: Duration,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerError {
    Configuration(String),
    Ingress(IngressError),
    StaleSource(InputId),
    Clock(String),
    Schedule(String),
}
#[derive(Debug)]
pub struct ProgramPacket {
    pub tick: OutputTick,
    pub frame: Result<RgbaFrame, WorkerError>,
}
#[derive(Debug, Default, Clone)]
pub struct WorkerStats {
    pub scheduled_ticks: u64,
    pub missed_deadlines: u64,
    pub rendered_frames: u64,
    pub unavailable_ticks: u64,
    pub last_output_error: Option<WorkerError>,
    pub output_queue_drops: u64,
    pub rejected_input_frames: u64,
    pub last_input_error: Option<(InputId, IngressError)>,
    pub terminal_error: Option<WorkerError>,
}

pub struct OutputWorker {
    sender: SyncSender<(InputId, CameraFrame)>,
    pub receiver: Receiver<ProgramPacket>,
    stop: Arc<AtomicBool>,
    stats: Arc<Mutex<WorkerStats>>,
    thread: Option<JoinHandle<()>>,
}

impl OutputWorker {
    pub fn start(config: WorkerConfig) -> Result<Self, WorkerError> {
        let fail = |message: &str| WorkerError::Configuration(message.into());
        if config.channel_capacity == 0 || config.max_hold.is_zero() {
            return Err(fail(
                "Queue capacity and source hold duration must be positive",
            ));
        }
        let mut output = CaptureOutput::new(config.profile).map_err(WorkerError::Ingress)?;
        for source in &config.sources {
            if source.requested_rate.numerator == 0
                || source.requested_rate.denominator == 0
                || (source.cadence == CadencePolicy::Exact
                    && !source.requested_rate.equivalent(config.profile.frame_rate))
            {
                return Err(fail(
                    "Source cadence differs; explicit HoldLatest conversion is required",
                ));
            }
            output
                .add_source(source.input, source.queue_capacity)
                .map_err(WorkerError::Ingress)?;
        }
        for id in std::iter::once(config.program).chain(config.overlays.into_iter().flatten()) {
            if output.stats(id).is_none() {
                return Err(fail("Program or overlay source is not registered"));
            }
        }
        let now = capture_clock_now_ns().map_err(WorkerError::Clock)?;
        let delay = u64::try_from(config.start_delay.as_nanos())
            .map_err(|_| fail("Start delay overflow"))?;
        let max_hold = u64::try_from(config.max_hold.as_nanos())
            .map_err(|_| fail("Source hold duration overflow"))?;
        let start = now
            .checked_add(delay)
            .ok_or_else(|| fail("Start timestamp overflow"))?;
        let schedule = OutputSchedule::new(start, config.profile.frame_rate)
            .map_err(|error| WorkerError::Schedule(format!("{error:?}")))?;
        let (sender, input) = mpsc::sync_channel(config.channel_capacity);
        let (packets, receiver) = mpsc::sync_channel(config.channel_capacity);
        let stop = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(Mutex::new(WorkerStats::default()));
        let worker_stop = Arc::clone(&stop);
        let worker_stats = Arc::clone(&stats);
        let worker = thread::Builder::new()
            .name("reference-program-output".into())
            .spawn(move || {
                let result = run(
                    output,
                    schedule,
                    &config,
                    max_hold,
                    WorkerIo {
                        input: &input,
                        packets: &packets,
                        stop: &worker_stop,
                        stats: &worker_stats,
                    },
                );
                if let Err(error) = result {
                    worker_stats
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner())
                        .terminal_error = Some(error);
                }
            })
            .map_err(|error| fail(&format!("Cannot spawn output worker: {error}")))?;
        Ok(Self {
            sender,
            receiver,
            stop,
            stats,
            thread: Some(worker),
        })
    }

    /// Nonblocking submission. Full/disconnected returns the owned frame for an
    /// explicit caller retry/drop policy; this method never discards it silently.
    pub fn try_submit(
        &self,
        input: InputId,
        frame: CameraFrame,
    ) -> Result<(), TrySendError<(InputId, CameraFrame)>> {
        self.sender.try_send((input, frame))?;
        if let Some(worker) = &self.thread {
            worker.thread().unpark();
        }
        Ok(())
    }
    pub fn stats(&self) -> WorkerStats {
        self.stats
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }
    /// Wakes a sleeping worker immediately, then joins it. No queue draining or
    /// consumer cooperation is required. A currently running CPU render completes.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.thread.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
impl Drop for OutputWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

struct WorkerIo<'a> {
    input: &'a Receiver<(InputId, CameraFrame)>,
    packets: &'a SyncSender<ProgramPacket>,
    stop: &'a AtomicBool,
    stats: &'a Mutex<WorkerStats>,
}

fn run(
    mut output: CaptureOutput,
    mut schedule: OutputSchedule,
    config: &WorkerConfig,
    max_hold: u64,
    io: WorkerIo<'_>,
) -> Result<(), WorkerError> {
    let WorkerIo {
        input,
        packets,
        stop,
        stats,
    } = io;
    while !stop.load(Ordering::Relaxed) {
        // Limit ingress per pass so a continuously busy producer cannot starve
        // deadlines or cancellation. No channel operation blocks.
        let mut rejected = 0;
        let mut last_input_error = None;
        for _ in 0..config.channel_capacity {
            let Ok((id, frame)) = input.try_recv() else {
                break;
            };
            if let Err(error) = output.ingest(id, frame) {
                rejected += 1;
                last_input_error = Some((id, error));
            }
        }
        {
            let mut stats = stats.lock().unwrap_or_else(|poison| poison.into_inner());
            stats.rejected_input_frames += rejected;
            if last_input_error.is_some() {
                stats.last_input_error = last_input_error;
            }
        }
        let now = capture_clock_now_ns().map_err(WorkerError::Clock)?;
        let tick = schedule
            .poll(now)
            .map_err(|error| WorkerError::Schedule(format!("{error:?}")))?;
        if let Some(tick) = tick {
            let frame = output
                .render_at(tick.pts_ns, config.program, None, config.overlays)
                .map_err(WorkerError::Ingress)
                .and_then(|frame| {
                    let stale = std::iter::once(config.program)
                        .chain(config.overlays.into_iter().flatten())
                        .find(|id| {
                            output
                                .source_selected_pts_ns(*id)
                                .is_some_and(|pts| tick.pts_ns.saturating_sub(pts) > max_hold)
                        });
                    match stale {
                        Some(id) => Err(WorkerError::StaleSource(id)),
                        None => Ok(frame),
                    }
                });
            let rendered = frame.is_ok();
            let output_error = frame.as_ref().err().cloned();
            let packet = ProgramPacket { tick, frame };
            let sent = packets.try_send(packet);
            let mut stats = stats.lock().unwrap_or_else(|poison| poison.into_inner());
            stats.last_output_error = output_error;
            stats.scheduled_ticks += 1;
            stats.missed_deadlines += tick.skipped_deadlines;
            if rendered {
                stats.rendered_frames += 1;
            } else {
                stats.unavailable_ticks += 1;
            }
            match sent {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => stats.output_queue_drops += 1,
                Err(TrySendError::Disconnected(_)) => return Ok(()),
            }
        } else {
            let next = schedule
                .next_deadline_ns()
                .map_err(|error| WorkerError::Schedule(format!("{error:?}")))?;
            thread::park_timeout(Duration::from_nanos(next.saturating_sub(now)));
        }
    }
    Ok(())
}
