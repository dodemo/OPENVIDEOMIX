#![cfg(feature = "gstreamer-capture")]
use openvideomix_core::{camera::FrameRate, video_profile::VideoProfile};
use openvideomix_desktop::{
    capture::{capture_clock_now_ns, CameraFrame},
    output_worker::{CadencePolicy, OutputWorker, WorkerConfig, WorkerError, WorkerSource},
};
use std::{
    thread,
    time::{Duration, Instant},
};
fn config() -> WorkerConfig {
    WorkerConfig {
        profile: VideoProfile {
            width: 2,
            height: 1,
            ..VideoProfile::default()
        },
        sources: vec![WorkerSource {
            input: 1,
            requested_rate: FrameRate::PAL_50,
            cadence: CadencePolicy::Exact,
            queue_capacity: 3,
        }],
        program: 1,
        overlays: [None; 4],
        channel_capacity: 2,
        max_hold: Duration::from_millis(100),
        start_delay: Duration::from_millis(20),
    }
}
fn frame(pts: u64) -> CameraFrame {
    CameraFrame {
        width: 2,
        height: 1,
        rgba: vec![200, 0, 0, 255, 200, 0, 0, 255],
        pts_ns: Some(0),
        capture_pts_ns: Some(pts),
        duration_ns: None,
    }
}
#[test]
fn rate_conversion_requires_explicit_opt_in_and_routes_must_exist() {
    let mut c = config();
    c.sources[0].requested_rate = FrameRate::new(30, 1).unwrap();
    assert!(matches!(
        OutputWorker::start(c),
        Err(WorkerError::Configuration(_))
    ));
    let mut c = config();
    c.program = 99;
    assert!(matches!(
        OutputWorker::start(c),
        Err(WorkerError::Configuration(_))
    ));
    let mut c = config();
    c.channel_capacity = 0;
    assert!(matches!(
        OutputWorker::start(c),
        Err(WorkerError::Configuration(_))
    ));
    let mut c = config();
    c.sources[0].requested_rate = FrameRate::new(30, 1).unwrap();
    c.sources[0].cadence = CadencePolicy::HoldLatest;
    let mut worker = OutputWorker::start(c).unwrap();
    worker.stop();
}
#[test]
fn worker_emits_timestamped_program_then_reports_stale_source() {
    let mut worker = OutputWorker::start(config()).unwrap();
    let pts = capture_clock_now_ns().unwrap();
    assert!(worker.try_submit(1, frame(pts)).is_ok());
    // A queued future frame must not hide the age of the frame actually held.
    assert!(worker.try_submit(1, frame(pts + 3_600_000_000_000)).is_ok());
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_frame = false;
    let mut saw_stale = false;
    let mut last_tick = None;
    while Instant::now() < deadline && !saw_stale {
        let packet = worker
            .receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        if let Some((index, pts)) = last_tick {
            assert!(packet.tick.index > index);
            assert_eq!(
                packet.tick.pts_ns - pts,
                (packet.tick.index - index) * 20_000_000
            );
        }
        last_tick = Some((packet.tick.index, packet.tick.pts_ns));
        match packet.frame {
            Ok(frame) => {
                assert_eq!(frame.pixels[0], 200);
                saw_frame = true
            }
            Err(WorkerError::StaleSource(1)) => saw_stale = true,
            _ => {}
        }
    }
    worker.stop();
    assert!(saw_frame && saw_stale);
    let stats = worker.stats();
    assert!(stats.rendered_frames > 0);
    assert!(stats.unavailable_ticks > 0);
    assert!(matches!(
        stats.last_output_error,
        Some(WorkerError::StaleSource(1))
    ));
    assert!(stats.terminal_error.is_none());
}
#[test]
fn full_output_channel_drops_explicitly_and_never_blocks_stop() {
    let mut c = config();
    c.channel_capacity = 1;
    let mut worker = OutputWorker::start(c).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while worker.stats().scheduled_ticks < 3 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    worker.stop();
    let stats = worker.stats();
    assert!(stats.scheduled_ticks >= 3);
    assert!(stats.output_queue_drops >= 2);
    assert_eq!(worker.receiver.try_iter().count(), 1);
    assert!(worker.try_submit(1, frame(0)).is_err());
}
#[test]
fn sleeping_worker_stops_without_waiting_for_long_deadline() {
    let mut c = config();
    c.start_delay = Duration::from_secs(3600);
    let mut worker = OutputWorker::start(c).unwrap();
    let started = Instant::now();
    worker.stop();
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(worker.stats().scheduled_ticks, 0);
}

#[test]
fn rejected_ingress_is_observable_without_blocking_a_sleeping_worker() {
    let mut c = config();
    c.start_delay = Duration::from_secs(3600);
    let mut worker = OutputWorker::start(c).unwrap();
    assert!(worker
        .try_submit(99, frame(capture_clock_now_ns().unwrap()))
        .is_ok());
    let deadline = Instant::now() + Duration::from_secs(5);
    while worker.stats().rejected_input_frames == 0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    worker.stop();
    let stats = worker.stats();
    assert_eq!(stats.rejected_input_frames, 1);
    assert!(matches!(
        stats.last_input_error,
        Some((
            99,
            openvideomix_desktop::capture_output::IngressError::UnknownInput(99)
        ))
    ));
    assert_eq!(stats.scheduled_ticks, 0);
}

#[test]
fn actual_synthetic_session_and_worker_share_the_same_clock_epoch() {
    use openvideomix_desktop::capture::{CameraEvent, CameraSession};
    let session = CameraSession::start_synthetic();
    let captured = match session
        .receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
    {
        CameraEvent::Frame(frame) => frame,
        CameraEvent::Error(error) => panic!("{error}"),
    };
    let expected = captured.rgba.clone();
    let mut c = config();
    // Explicit experimental mode; preserve the shared production default.
    c.profile = VideoProfile {
        width: 640,
        height: 360,
        frame_rate: FrameRate::new(30, 1).unwrap(),
    };
    c.sources[0].requested_rate = c.profile.frame_rate;
    c.max_hold = Duration::from_secs(5);
    let mut worker = OutputWorker::start(c).unwrap();
    assert!(worker.try_submit(1, captured).is_ok());
    let packet = worker
        .receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    assert_eq!(packet.frame.unwrap().pixels, expected);
    worker.stop();
    assert!(worker.stats().terminal_error.is_none());
}
