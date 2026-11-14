//! Desktop-facing service: actual Program frames and explicit test-tone recording.
//! Capture and output run independently of GUI polling. No microphone is implied.
use crate::{
    output_worker::{
        CadencePolicy, OutputIngress, OutputRouting, OutputWorker, ProgramPacket, WorkerConfig,
        WorkerError, WorkerSource, WorkerStats,
    },
    program_recording::{ProgramRecorder, RecordingStats},
};
use openvideomix_core::{
    reference_compositor::RgbaFrame,
    video_profile::{VideoProfile, DEFAULT_AUDIO_SAMPLE_RATE},
    InputId,
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Default)]
pub enum RecordingState {
    #[default]
    Idle,
    Starting(PathBuf),
    Recording {
        path: PathBuf,
        submitted: u64,
        gaps: u64,
    },
    Saved(RecordingStats),
    Error(String),
}
impl RecordingState {
    pub fn active(&self) -> bool {
        matches!(self, Self::Starting(_) | Self::Recording { .. })
    }
}
#[derive(Default, Clone)]
pub struct StudioView {
    pub latest: Option<(u64, Arc<RgbaFrame>)>,
    pub program_error: Option<String>,
    pub stats: WorkerStats,
    pub recording: RecordingState,
    pub recording_queue_drops: u64,
}
enum RecordControl {
    Start(PathBuf),
    Stop,
}
pub struct DesktopProduction {
    ingress: OutputIngress,
    routing: OutputRouting,
    view: Arc<Mutex<StudioView>>,
    gain: Arc<Mutex<f32>>,
    approved_directory: Arc<Mutex<Option<PathBuf>>>,
    commands: SyncSender<RecordControl>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}
impl DesktopProduction {
    pub fn start(inputs: &[InputId]) -> Result<Self, String> {
        let first = *inputs.first().ok_or("No production inputs")?;
        let profile = VideoProfile::default();
        let worker = OutputWorker::start(WorkerConfig {
            profile,
            sources: inputs
                .iter()
                .map(|id| WorkerSource {
                    input: *id,
                    requested_rate: profile.frame_rate,
                    cadence: CadencePolicy::Exact,
                    queue_capacity: 12,
                })
                .collect(),
            program: first,
            overlays: [None; 4],
            channel_capacity: 16,
            max_hold: Duration::from_millis(500),
            start_delay: Duration::from_millis(100),
            input_latency: Duration::from_millis(100),
        })
        .map_err(|e| format!("Output worker: {e:?}"))?;
        let ingress = worker.ingress().ok_or("Output worker has no ingress")?;
        let routing = worker.routing();
        let view = Arc::new(Mutex::new(StudioView::default()));
        let gain = Arc::new(Mutex::new(0.8));
        let stop = Arc::new(AtomicBool::new(false));
        let (commands, requests) = mpsc::sync_channel(2);
        let recording_active = Arc::new(AtomicBool::new(false));
        let (frames, recording_frames) = mpsc::sync_channel(2);
        let output_view = Arc::clone(&view);
        let output_stop = Arc::clone(&stop);
        let output_recording = Arc::clone(&recording_active);
        let output_thread = thread::Builder::new()
            .name("desktop-program-monitor".into())
            .spawn(move || {
                let mut worker = worker;
                let trace = std::env::var_os("OPENVIDEOMIX_TRACE_OUTPUT").is_some();
                let mut last_trace = Instant::now();
                while !output_stop.load(Ordering::Relaxed) {
                    match worker.receiver.recv_timeout(Duration::from_millis(20)) {
                        Ok(packet) => publish_program(
                            packet,
                            &output_view,
                            &frames,
                            output_recording.load(Ordering::Relaxed),
                        ),
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                    output_view.lock().unwrap_or_else(|p| p.into_inner()).stats = worker.stats();
                    if trace && last_trace.elapsed() >= Duration::from_secs(1) {
                        eprintln!("Program output {:?}", worker.stats());
                        last_trace = Instant::now();
                    }
                }
                worker.stop();
            })
            .map_err(|e| format!("Cannot start Program monitor: {e}"))?;
        let worker_view = Arc::clone(&view);
        let worker_gain = Arc::clone(&gain);
        let worker_stop = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("desktop-program-consumer".into())
            .spawn(move || {
                let mut recorder: Option<ProgramRecorder> = None;
                let mut origin = None;
                let mut submitted = 0_u64;
                let mut sample_index = 0_u64;
                let mut path = PathBuf::new();
                while !worker_stop.load(Ordering::Relaxed) {
                    for _ in 0..2 {
                        let Ok(command) = requests.try_recv() else {
                            break;
                        };
                        match command {
                            RecordControl::Start(requested) => {
                                if recorder.is_some() {
                                    continue;
                                }
                                match ProgramRecorder::start(&requested, profile) {
                                    Ok(session) => {
                                        // Discard packets from the previous session before
                                        // publishing a fresh recording origin.
                                        while recording_frames.try_recv().is_ok() {}
                                        recording_active.store(true, Ordering::Relaxed);
                                        worker_view.lock().unwrap_or_else(|p| p.into_inner()).recording_queue_drops = 0;
                                        recorder = Some(session);
                                        origin = None;
                                        submitted = 0;
                                        sample_index = 0;
                                        path = requested.clone();
                                        worker_view
                                            .lock()
                                            .unwrap_or_else(|p| p.into_inner())
                                            .recording = RecordingState::Starting(requested);
                                    }
                                    Err(error) => {
                                        worker_view
                                            .lock()
                                            .unwrap_or_else(|p| p.into_inner())
                                            .recording = RecordingState::Error(error)
                                    }
                                }
                            }
                            RecordControl::Stop => {
                                recording_active.store(false, Ordering::Relaxed);
                                if let Some(session) = recorder.take() {
                                    let result = session.finish(Duration::from_secs(10));
                                    worker_view
                                        .lock()
                                        .unwrap_or_else(|p| p.into_inner())
                                        .recording = match result {
                                        Ok(stats) => RecordingState::Saved(stats),
                                        Err(error) => RecordingState::Error(error),
                                    };
                                }
                            }
                        }
                    }
                    let packet = match recording_frames.recv_timeout(Duration::from_millis(20)) {
                        Ok(packet) => packet,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    match packet.1 {
                        Ok(frame) => {
                            if let Some(session) = recorder.as_mut() {
                                let first = *origin.get_or_insert(packet.0);
                                let index = packet.0 - first;
                                let volume = *worker_gain.lock().unwrap_or_else(|p| p.into_inner());
                                let result = (|| {
                                    let count = session.required_audio_samples_for_frame(index)?;
                                    if count > u64::from(DEFAULT_AUDIO_SAMPLE_RATE) {
                                        return Err(
                                            "Audio interval exceeds one-second transaction".into(),
                                        );
                                    }
                                    let mut pcm = Vec::with_capacity(count as usize * 2);
                                    for _ in 0..count {
                                        let value =
                                            (sample_index as f64 * std::f64::consts::TAU * 1000.0
                                                / f64::from(DEFAULT_AUDIO_SAMPLE_RATE))
                                            .sin()
                                                as f32
                                                * 0.5
                                                * volume;
                                        pcm.extend([value, value]);
                                        sample_index += 1;
                                    }
                                    session.wait_for_frame_capacity(
                                        index,
                                        // A complex first/key frame can take longer than one
                                        // video tick. Wait only on this bounded consumer;
                                        // capture and Program scheduling remain independent.
                                        Duration::from_secs(1),
                                    )?;
                                    session.write_frame_at(index, &frame, &pcm)
                                })();
                                match result {
                                    Ok(()) => {
                                        submitted += 1;
                                        worker_view
                                            .lock()
                                            .unwrap_or_else(|p| p.into_inner())
                                            .recording = RecordingState::Recording {
                                            path: path.clone(),
                                            submitted,
                                            gaps: index + 1 - submitted,
                                        };
                                    }
                                    Err(error) => {
                                        recording_active.store(false, Ordering::Relaxed);
                                        if let Some(session) = recorder.take() {
                                            let _ = session.finish(Duration::from_secs(10));
                                        }
                                        worker_view
                                            .lock()
                                            .unwrap_or_else(|p| p.into_inner())
                                            .recording = RecordingState::Error(format!(
                                            "{error}; partial file: {}",
                                            path.display()
                                        ));
                                    }
                                }
                            }
                        }
                        Err(error) => {
                            if let (Some(first), Some(session)) = (origin, recorder.as_ref()) {
                                let gap = session.required_audio_samples_for_frame(packet.0 - first);
                                if gap.is_err() || gap.is_ok_and(|samples| samples > u64::from(DEFAULT_AUDIO_SAMPLE_RATE)) {
                                    recording_active.store(false, Ordering::Relaxed);
                                    if let Some(session) = recorder.take() {
                                        let _ = session.finish(Duration::from_secs(10));
                                    }
                                    worker_view.lock().unwrap_or_else(|p| p.into_inner()).recording =
                                        RecordingState::Error(format!(
                                            "Program unavailable beyond bounded audio interval: {error:?}; partial file: {}",
                                            path.display()
                                        ));
                                }
                            }
                        }
                    }
                }
                recording_active.store(false, Ordering::Relaxed);
                if let Some(session) = recorder {
                    let result = session.finish(Duration::from_secs(10));
                    worker_view
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .recording = match result {
                        Ok(stats) => RecordingState::Saved(stats),
                        Err(error) => RecordingState::Error(error),
                    };
                }
            });
        let thread = match thread {
            Ok(thread) => thread,
            Err(error) => {
                stop.store(true, Ordering::Relaxed);
                let _ = output_thread.join();
                return Err(format!("Cannot start recording consumer: {error}"));
            }
        };
        Ok(Self {
            ingress,
            routing,
            view,
            gain,
            approved_directory: Arc::new(Mutex::new(None)),
            commands,
            stop,
            threads: vec![output_thread, thread],
        })
    }
    pub fn ingress(&self) -> OutputIngress {
        self.ingress.clone()
    }
    pub fn view(&self) -> StudioView {
        self.view.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
    pub fn route(
        &self,
        program: Option<InputId>,
        overlay: Option<InputId>,
        master_gain: f32,
        muted: bool,
    ) {
        self.routing
            .set(program.unwrap_or(0), [overlay, None, None, None]);
        *self.gain.lock().unwrap_or_else(|p| p.into_inner()) = if muted {
            0.0
        } else {
            master_gain.clamp(0.0, 1.0)
        };
    }
    /// Only an operator-approved directory is writable through recording intents.
    pub fn approve_recording_directory(&self, directory: &std::path::Path) -> Result<(), String> {
        let directory = directory
            .canonicalize()
            .map_err(|error| format!("Recording directory: {error}"))?;
        if !directory.is_dir() {
            return Err("Recording destination is not a directory".into());
        }
        *self
            .approved_directory
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(directory);
        Ok(())
    }

    pub fn start_test_recording(&self, path: PathBuf) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or("Recording path has no directory")?
            .canonicalize()
            .map_err(|error| format!("Recording directory: {error}"))?;
        if self
            .approved_directory
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            != Some(&parent)
        {
            return Err("Recording directory has not been approved by the operator".into());
        }
        self.commands
            .try_send(RecordControl::Start(path))
            .map_err(|e| format!("Recording command: {e}"))
    }
    pub fn stop_recording(&self) -> Result<(), String> {
        self.commands
            .try_send(RecordControl::Stop)
            .map_err(|e| format!("Recording command: {e}"))
    }
}
impl Drop for DesktopProduction {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for worker in self.threads.drain(..) {
            let _ = worker.join();
        }
    }
}

type RecordingPacket = (u64, Result<Arc<RgbaFrame>, WorkerError>);

/// The monitor always receives the latest actual Program. A full encoder
/// queue drops/counts only recording delivery, never monitor publication.
fn publish_program(
    packet: ProgramPacket,
    view: &Mutex<StudioView>,
    recording: &SyncSender<RecordingPacket>,
    recording_active: bool,
) {
    let frame = packet.frame.map(Arc::new);
    let mut view = view.lock().unwrap_or_else(|p| p.into_inner());
    match &frame {
        Ok(frame) => {
            view.latest = Some((packet.tick.index, Arc::clone(frame)));
            view.program_error = None;
        }
        Err(error) => {
            view.latest = None;
            view.program_error = Some(format!("{error:?}"));
        }
    }
    if recording_active
        && matches!(
            recording.try_send((packet.tick.index, frame)),
            Err(mpsc::TrySendError::Full(_))
        )
    {
        view.recording_queue_drops += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openvideomix_core::output_schedule::OutputTick;

    #[test]
    fn blocked_encoder_does_not_block_or_rewind_the_program_monitor() {
        let (sender, receiver) = mpsc::sync_channel(2);
        let view = Mutex::new(StudioView::default());
        // Keep the receiver alive but deliberately never consume a packet.
        // Publication must still reach the newest frame despite saturation.
        for index in 0..60 {
            publish_program(
                ProgramPacket {
                    tick: OutputTick {
                        index,
                        pts_ns: index * 20_000_000,
                        skipped_deadlines: 0,
                        lateness_ns: 0,
                    },
                    frame: Ok(RgbaFrame {
                        width: 1,
                        height: 1,
                        pixels: vec![index as u8, 0, 0, 255],
                    }),
                },
                &view,
                &sender,
                true,
            );
        }
        let view = view.lock().unwrap();
        let (index, frame) = view.latest.as_ref().unwrap();
        assert_eq!((*index, frame.pixels[0]), (59, 59));
        assert_eq!(view.recording_queue_drops, 58);
        let (_, first) = receiver.try_recv().unwrap();
        assert_eq!(first.unwrap().pixels[0], 0);
        assert!(view.program_error.is_none());
    }
}
