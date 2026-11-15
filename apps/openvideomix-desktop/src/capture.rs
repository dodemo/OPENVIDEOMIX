//! Minimal real USB-camera ingestion via GStreamer.
//!
//! This opt-in proof of concept uses autovideosrc, so it selects a system
//! default camera rather than a specific persistent device ID. It is not yet
//! a production 1080p50 or multi-camera capture implementation.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender, TrySendError},
    Arc, OnceLock,
};
use std::thread::{self, JoinHandle};

use gst::prelude::*;
use gst_video::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;

/// Every capture pipeline explicitly uses the same system clock and epoch.
/// This maps pipeline-local running time; it does not discipline device clocks
/// or measure the physical exposure time of a camera frame.
pub(crate) struct CaptureTimeline {
    pub(crate) clock: gst::Clock,
    pub(crate) epoch_ns: u64,
}

impl CaptureTimeline {
    pub(crate) fn shared() -> &'static Self {
        static TIMELINE: OnceLock<CaptureTimeline> = OnceLock::new();
        TIMELINE.get_or_init(|| {
            let clock = gst::SystemClock::obtain();
            let epoch_ns = clock.time().nseconds();
            Self { clock, epoch_ns }
        })
    }
}

/// Current time in the same epoch used by CameraFrame::capture_pts_ns.
/// This clock reading is not a measured capture/output frame rate.
pub fn capture_clock_now_ns() -> Result<u64, String> {
    gst::init().map_err(|error| format!("GStreamer clock initialization: {error}"))?;
    let timeline = CaptureTimeline::shared();
    timeline
        .clock
        .time()
        .nseconds()
        .checked_sub(timeline.epoch_ns)
        .ok_or_else(|| "Capture clock moved before its epoch".into())
}

/// Convert through the sample segment: raw PTS need not equal running time.
/// Missing metadata, out-of-segment PTS and arithmetic overflow stay unknown.
pub(crate) fn map_capture_pts_ns(
    pts: Option<gst::ClockTime>,
    segment: Option<&gst::Segment>,
    base_time: Option<gst::ClockTime>,
    epoch_ns: u64,
) -> Option<u64> {
    let running = segment?
        .downcast_ref::<gst::ClockTime>()?
        .to_running_time(pts?)?;
    base_time?
        .nseconds()
        .checked_add(running.nseconds())?
        .checked_sub(epoch_ns)
}

/// Always return an active pipeline to NULL, including startup/error exits.
struct CapturePipeline(gst::Pipeline);
impl Drop for CapturePipeline {
    fn drop(&mut self) {
        let _ = self.0.set_state(gst::State::Null);
    }
}

#[derive(Clone)]
pub struct CameraFrame {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    /// GStreamer buffer PTS; relative to the source pipeline timebase.
    /// None when the capture backend does not provide timestamps.
    pub pts_ns: Option<u64>,
    /// Segment-adjusted presentation time in the shared capture-clock epoch.
    /// Not a measured physical capture time, output FPS or drift correction.
    pub capture_pts_ns: Option<u64>,
    /// Advertised buffer duration, not a measured end-to-end frame period.
    pub duration_ns: Option<u64>,
}

pub enum CameraEvent {
    Frame(CameraFrame),
    Error(String),
}

pub struct CameraSession {
    pub receiver: Receiver<CameraEvent>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl CameraSession {
    pub fn start() -> Self {
        Self::start_source("autovideosrc")
    }

    pub fn start_synthetic() -> Self {
        Self::start_source("videotestsrc is-live=true pattern=ball")
    }

    fn start_source(source: &'static str) -> Self {
        Self::start_configured(source, None)
    }

    /// A full-HD test source feeds production directly; GUI frame polling is
    /// an independent subscriber. Pattern is selected from a closed set.
    pub fn start_production_test(
        input: openvideomix_core::InputId,
        pattern: u32,
        target: crate::output_worker::OutputIngress,
    ) -> Self {
        let source = match pattern % 4 {
            0 => "videotestsrc is-live=true pattern=smpte",
            1 => "videotestsrc is-live=true pattern=solid-color foreground-color=0xffc02020",
            2 => "videotestsrc is-live=true pattern=solid-color foreground-color=0xff2040c0",
            _ => "videotestsrc is-live=true pattern=checkers-8",
        };
        Self::start_configured(source, Some((input, target)))
    }

    fn start_configured(
        source: &'static str,
        target: Option<(
            openvideomix_core::InputId,
            crate::output_worker::OutputIngress,
        )>,
    ) -> Self {
        let (sender, receiver) = mpsc::sync_channel(3);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            if let Err(message) =
                run_capture_configured(&sender, &worker_stop, source, target.as_ref())
            {
                // Frames may fill the bounded queue. Keep the terminal error
                // pending until the GUI drains a slot, unless capture is
                // being stopped. A failed try_send would hide the failure.
                let mut event = CameraEvent::Error(message);
                while !worker_stop.load(Ordering::Relaxed) {
                    match sender.try_send(event) {
                        Ok(()) | Err(TrySendError::Disconnected(_)) => break,
                        Err(TrySendError::Full(pending)) => {
                            event = pending;
                            thread::sleep(std::time::Duration::from_millis(10));
                        }
                    }
                }
            }
        });
        Self {
            receiver,
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for CameraSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
fn run_capture(
    sender: &SyncSender<CameraEvent>,
    stop: &AtomicBool,
    source: &str,
) -> Result<(), String> {
    run_capture_configured(sender, stop, source, None)
}

fn run_capture_configured(
    sender: &SyncSender<CameraEvent>,
    stop: &AtomicBool,
    source: &str,
    target: Option<&(
        openvideomix_core::InputId,
        crate::output_worker::OutputIngress,
    )>,
) -> Result<(), String> {
    gst::init().map_err(|e| format!("GStreamer: {e}"))?;
    let timeline = CaptureTimeline::shared();

    // A conservative working format is used until device capability
    // enumeration and hardware-dependent negotiation are implemented.
    // 'autovideosrc' chooses a platform-specific camera source.
    let pipeline_description = if target.is_some() {
        let profile = openvideomix_core::video_profile::VideoProfile::default();
        format!("{source} ! video/x-raw,width={},height={},framerate={}/{} ! videoconvert ! video/x-raw,format=RGBA ! appsink name=framesink emit-signals=false sync=false max-buffers=2 drop=true",profile.width,profile.height,profile.frame_rate.numerator,profile.frame_rate.denominator)
    } else {
        format!(
            "{source} ! videoconvert ! videoscale ! videorate ! \
         video/x-raw,format=RGBA,width=640,height=360,framerate=30/1 ! \
         appsink name=framesink emit-signals=false sync=false max-buffers=2 drop=true"
        )
    };
    let element = gst::parse::launch(&pipeline_description)
        .map_err(|e| format!("Non se puido crear o pipeline de webcam: {e}"))?;

    let pipeline = element
        .dynamic_cast::<gst::Pipeline>()
        .map_err(|_| "O pipeline de captura non e unha Pipeline".to_string())?;
    let guard = CapturePipeline(pipeline);
    let pipeline = &guard.0;
    pipeline.use_clock(Some(&timeline.clock));
    let sink = pipeline
        .by_name("framesink")
        .and_then(|element| element.dynamic_cast::<gst_app::AppSink>().ok())
        .ok_or_else(|| "Non se atopou o AppSink de captura".to_string())?;

    pipeline
        .set_state(gst::State::Playing)
        .map_err(|e| format!("Non se puido iniciar a webcam: {e}"))?;

    let bus = pipeline
        .bus()
        .ok_or_else(|| "Pipeline sen bus de mensaxes".to_string())?;

    loop {
        if stop.load(Ordering::Relaxed) {
            break Ok(());
        }
        if let Some(message) = bus.timed_pop_filtered(
            gst::ClockTime::ZERO,
            &[gst::MessageType::Error, gst::MessageType::Eos],
        ) {
            match message.view() {
                gst::MessageView::Error(error) => {
                    break Err(format!("Erro de captura: {}", error.error()));
                }
                gst::MessageView::Eos(..) => {
                    break Err("A fonte de webcam rematou a captura".to_string());
                }
                _ => {}
            }
        }

        let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_mseconds(100)) else {
            if sink.is_eos() {
                break Err("A webcam deixou de enviar fotogramas".to_string());
            }
            continue;
        };
        let Some(caps) = sample.caps() else {
            continue;
        };
        let Ok(info) = gst_video::VideoInfo::from_caps(caps) else {
            continue;
        };
        if info.format() != gst_video::VideoFormat::Rgba {
            continue;
        }
        let width = info.width() as usize;
        let height = info.height() as usize;
        if width == 0 || height == 0 {
            continue;
        }
        let Some(buffer) = sample.buffer() else {
            continue;
        };
        let Ok(video_frame) = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info)
        else {
            continue;
        };
        let Ok(plane) = video_frame.plane_data(0) else {
            continue;
        };
        let stride = video_frame.plane_stride()[0];
        let Some(rgba) = copy_packed_rgba(plane, width, height, stride) else {
            continue;
        };

        let pts_ns = buffer.pts().map(|time| time.nseconds());
        let duration_ns = buffer.duration().map(|time| time.nseconds());
        let capture_pts_ns = map_capture_pts_ns(
            buffer.pts(),
            sample.segment(),
            pipeline.base_time(),
            timeline.epoch_ns,
        );
        let frame = CameraFrame {
            width,
            height,
            rgba,
            pts_ns,
            capture_pts_ns,
            duration_ns,
        };
        if let Some((input, target)) = target {
            // Output has its own bounded ingress. A slow/paused GUI cannot
            // throttle production. Count failures in the desktop service next.
            let _ = target.try_submit(*input, frame.clone());
        }
        match sender.try_send(CameraEvent::Frame(frame)) {
            Ok(()) | Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => break Ok(()),
        }
    }
}

/// Copy an RGBA frame row by row, ignoring padding bytes and rejecting any
/// malformed or negative video stride rather than exposing corrupt pixels.
fn copy_packed_rgba(data: &[u8], width: usize, height: usize, stride: i32) -> Option<Vec<u8>> {
    let row_bytes = width.checked_mul(4)?;
    let stride = usize::try_from(stride).ok()?;
    if row_bytes == 0 || height == 0 || stride < row_bytes {
        return None;
    }
    let final_row = stride.checked_mul(height.checked_sub(1)?)?;
    let minimum_size = final_row.checked_add(row_bytes)?;
    if data.len() < minimum_size {
        return None;
    }
    let mut pixels = vec![0; row_bytes.checked_mul(height)?];
    for (row, output) in pixels.chunks_exact_mut(row_bytes).enumerate() {
        let start = row.checked_mul(stride)?;
        output.copy_from_slice(&data[start..start + row_bytes]);
    }
    Some(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn shared_timestamp_mapping_respects_segment_and_checked_arithmetic() {
        gst::init().unwrap();
        let mut segment = gst::FormattedSegment::<gst::ClockTime>::new();
        segment.set_start(gst::ClockTime::from_seconds(1));
        segment.set_base(gst::ClockTime::from_seconds(2));
        let pts = Some(gst::ClockTime::from_mseconds(1500));
        let base = Some(gst::ClockTime::from_seconds(10));
        assert_eq!(
            map_capture_pts_ns(pts, Some(segment.upcast_ref()), base, 9_000_000_000),
            Some(3_500_000_000)
        );
        assert_eq!(
            map_capture_pts_ns(None, Some(segment.upcast_ref()), base, 0),
            None
        );
        assert_eq!(map_capture_pts_ns(pts, None, base, 0), None);
        assert_eq!(
            map_capture_pts_ns(pts, Some(segment.upcast_ref()), None, 0),
            None
        );
        assert_eq!(
            map_capture_pts_ns(
                Some(gst::ClockTime::ZERO),
                Some(segment.upcast_ref()),
                base,
                0
            ),
            None
        );
        assert_eq!(
            map_capture_pts_ns(pts, Some(segment.upcast_ref()), base, u64::MAX),
            None
        );
        assert_eq!(
            map_capture_pts_ns(
                pts,
                Some(segment.upcast_ref()),
                Some(gst::ClockTime::from_nseconds(u64::MAX - 1)),
                0
            ),
            None
        );
    }

    fn receive_frame(session: &CameraSession) -> CameraFrame {
        match session.receiver.recv_timeout(Duration::from_secs(5)) {
            Ok(CameraEvent::Frame(frame)) => frame,
            Ok(CameraEvent::Error(message)) => panic!("Synthetic source failed: {message}"),
            Err(error) => panic!("No synthetic frame: {error}"),
        }
    }

    #[test]
    fn staggered_sources_preserve_start_offset_on_shared_timeline() {
        let first = CameraSession::start_synthetic();
        let first_frame = receive_frame(&first);
        let before_second = CaptureTimeline::shared().clock.time().nseconds();
        // A later session must retain its actual pipeline start offset;
        // independently subtracting each source's first PTS would erase it.
        let second = CameraSession::start_synthetic();
        let second_frame = receive_frame(&second);
        let timeline = CaptureTimeline::shared();
        let first_pts = first_frame.capture_pts_ns.expect("shared first PTS");
        let second_pts = second_frame.capture_pts_ns.expect("shared second PTS");
        assert!(second_pts >= before_second - timeline.epoch_ns);
        assert!(second_pts > first_pts);
        let next = receive_frame(&second);
        assert!(next.capture_pts_ns.unwrap() > second_pts);
        assert!(first_frame.pts_ns.is_some() && second_frame.pts_ns.is_some());
        drop(first);
        assert!(receive_frame(&second).capture_pts_ns.unwrap() > second_pts);
    }

    #[test]
    fn packed_rgba_handles_row_padding_and_rejects_bad_strides() {
        let incomplete = [1, 2, 3, 4, 5, 6, 7, 8, 99, 99, 9, 10, 11, 12, 13, 14, 15];
        assert!(
            copy_packed_rgba(&incomplete, 2, 2, 10).is_none(),
            "An incomplete second row must be rejected"
        );
        let padded = [
            1, 2, 3, 4, 5, 6, 7, 8, 99, 99, 9, 10, 11, 12, 13, 14, 15, 16,
        ];
        assert_eq!(
            copy_packed_rgba(&padded, 2, 2, 10),
            Some(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16])
        );
        assert!(copy_packed_rgba(&padded, 2, 2, -1).is_none());
        assert!(copy_packed_rgba(&padded, 2, 2, 7).is_none());
    }

    #[test]
    fn capture_failure_is_reported_even_with_a_full_frame_queue() {
        // An error must not be silently discarded when three pending frames
        // occupy the bounded channel. Deliberately use an invalid source.
        let (sender, receiver) = mpsc::sync_channel(3);
        for _ in 0..3 {
            sender
                .send(CameraEvent::Frame(CameraFrame {
                    width: 1,
                    height: 1,
                    rgba: vec![0, 0, 0, 255],
                    pts_ns: None,
                    capture_pts_ns: None,
                    duration_ns: None,
                }))
                .expect("frame queue should accept three items");
        }
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            if let Err(message) = run_capture(
                &sender,
                &worker_stop,
                "this_gstreamer_element_does_not_exist",
            ) {
                let mut event = CameraEvent::Error(message);
                while !worker_stop.load(Ordering::Relaxed) {
                    match sender.try_send(event) {
                        Ok(()) | Err(TrySendError::Disconnected(_)) => break,
                        Err(TrySendError::Full(pending)) => {
                            event = pending;
                            thread::sleep(Duration::from_millis(10));
                        }
                    }
                }
            }
        });

        // Drain the pre-filled frames and allow the worker to publish its
        // terminal error into the newly available channel capacity.
        for _ in 0..3 {
            assert!(matches!(
                receiver.recv_timeout(Duration::from_secs(2)),
                Ok(CameraEvent::Frame(_))
            ));
        }
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(5)),
            Ok(CameraEvent::Error(_))
        ));
        stop.store(true, Ordering::Relaxed);
        worker.join().expect("capture worker must terminate");
    }

    #[test]
    fn synthetic_capture_preserves_monotonic_source_pts() {
        let session = CameraSession::start_synthetic();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut last_pts = None;
        let mut observed = 0;
        while observed < 5 && Instant::now() < deadline {
            match session.receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(CameraEvent::Frame(frame)) => {
                    let pts = frame
                        .pts_ns
                        .expect("videotestsrc should provide a source PTS");
                    if let Some(previous) = last_pts {
                        assert!(pts > previous, "source timestamps must be monotonic");
                    }
                    last_pts = Some(pts);
                    observed += 1;
                }
                Ok(CameraEvent::Error(message)) => {
                    panic!("Synthetic timestamp test failed: {message}")
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        assert_eq!(observed, 5, "insufficient captured timestamped frames");
    }

    #[test]
    fn synthetic_full_hd_50fps_negotiates_exact_caps_and_pts() {
        // Validates only a software-generated GStreamer media path.
        // It does not establish USB camera support or measured output FPS.
        gst::init().expect("GStreamer initialization");
        let element = gst::parse::launch(
            "videotestsrc is-live=true num-buffers=20 pattern=ball ! \
             video/x-raw,format=RGBA,width=1920,height=1080,framerate=50/1 ! \
             appsink name=framesink sync=false max-buffers=2 drop=true",
        )
        .expect("synthetic 1080p50 pipeline");
        let pipeline = element
            .dynamic_cast::<gst::Pipeline>()
            .expect("test pipeline");
        let sink = pipeline
            .by_name("framesink")
            .expect("sink")
            .dynamic_cast::<gst_app::AppSink>()
            .expect("appsink");

        pipeline.set_state(gst::State::Playing).expect("start");
        let result = (|| -> Result<(), String> {
            let mut previous_pts = None;
            for _ in 0..3 {
                let sample = sink
                    .try_pull_sample(gst::ClockTime::from_seconds(5))
                    .ok_or_else(|| "no synthetic sample at 1080p50".to_string())?;
                let info = gst_video::VideoInfo::from_caps(
                    sample.caps().ok_or_else(|| "missing caps".to_string())?,
                )
                .map_err(|error| error.to_string())?;
                if info.width() != 1920
                    || info.height() != 1080
                    || info.fps() != gst::Fraction::new(50, 1)
                    || info.format() != gst_video::VideoFormat::Rgba
                {
                    return Err(format!("incorrect 1080p50 negotiated mode: {info:?}"));
                }
                let pts = sample
                    .buffer()
                    .and_then(|buffer| buffer.pts())
                    .ok_or_else(|| "missing 1080p50 PTS".to_string())?
                    .nseconds();
                if let Some(previous) = previous_pts {
                    if pts <= previous {
                        return Err("non-monotonic 1080p50 PTS".into());
                    }
                }
                previous_pts = Some(pts);
            }
            Ok(())
        })();
        let _ = pipeline.set_state(gst::State::Null);
        result.expect("1080p50 synthetic negotiation");
    }

    #[test]
    fn two_synthetic_sources_capture_concurrently() {
        // Two independent pipelines simulate distinct video inputs without
        // taking ownership of a physical webcam.
        let first = CameraSession::start_synthetic();
        let second = CameraSession::start_synthetic();

        for session in [&first, &second] {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut received = false;
            while Instant::now() < deadline {
                match session.receiver.recv_timeout(Duration::from_millis(250)) {
                    Ok(CameraEvent::Frame(frame)) => {
                        received = frame.width == 640
                            && frame.height == 360
                            && frame.rgba.len() == 640 * 360 * 4
                            && frame.pts_ns.is_some();
                        if received {
                            break;
                        }
                    }
                    Ok(CameraEvent::Error(message)) => {
                        panic!("Concurrent synthetic video source failed: {message}")
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            assert!(
                received,
                "An independently launched pipeline did not send video"
            );
        }
        // Shutting down one source must not prevent the other receiving video.
        drop(first);
        match second.receiver.recv_timeout(Duration::from_secs(3)) {
            Ok(CameraEvent::Frame(frame)) => assert_eq!(frame.rgba.len(), 640 * 360 * 4),
            Ok(CameraEvent::Error(message)) => panic!("Second pipeline failed: {message}"),
            Err(error) => panic!("Second pipeline stopped with first: {error}"),
        }
    }

    #[test]
    fn synthetic_video_produces_real_rgba_frames() {
        // No physical device required. This needs GStreamer base plugins.
        let (tx, rx) = mpsc::sync_channel(3);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            run_capture(&tx, &worker_stop, "videotestsrc is-live=true pattern=ball")
        });
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut valid_frame = false;
        while Instant::now() < deadline {
            match rx.recv_timeout(Duration::from_millis(250)) {
                Ok(CameraEvent::Frame(frame)) => {
                    valid_frame = frame.width == 640
                        && frame.height == 360
                        && frame.rgba.len() == 640 * 360 * 4;
                    if valid_frame {
                        break;
                    }
                }
                Ok(CameraEvent::Error(message)) => panic!("Pipeline de proba fallou: {message}"),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        stop.store(true, Ordering::Relaxed);
        let result = worker.join().expect("O worker non debe fallar");
        result.expect("O pipeline non debe fallar");
        assert!(valid_frame, "Non chegou un fotograma RGBA válido");
    }
}
