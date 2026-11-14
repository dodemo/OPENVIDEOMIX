#![cfg(feature = "gstreamer-capture")]
use openvideomix_desktop::{
    capture::capture_clock_now_ns,
    live_audio::{levels, AudioBlock, AudioInput, AudioSource, AudioState, AudioTimeline},
};
use std::time::{Duration, Instant};
fn block(pts_ns: u64, count: usize, value: f32) -> AudioBlock {
    AudioBlock {
        pts_ns,
        pcm: vec![value; count * 2],
        discontinuity: false,
    }
}
#[test]
fn delayed_start_discontinuity_mute_and_underflow_are_measured() {
    let mut timeline = AudioTimeline::new(1_000_000_000);
    let mut data = block(1_010_000_000, 480, 0.5);
    data.discontinuity = true;
    timeline.ingest(data).unwrap();
    let pcm = timeline.render(960, 0.5).unwrap();
    assert!(pcm[..960].iter().all(|v| *v == 0.0));
    assert!(pcm[960..].iter().all(|v| *v == 0.25));
    assert_eq!(timeline.stats.silence_samples, 480);
    assert_eq!(timeline.stats.discontinuities, 1);
    let (peak, rms) = levels(&pcm);
    assert!((peak + 12.0412).abs() < 0.001);
    assert!((rms + 15.0515).abs() < 0.001);
    timeline.ingest(block(1_020_000_000, 480, 0.5)).unwrap();
    assert!(timeline.render(480, 0.0).unwrap().iter().all(|v| *v == 0.0));
    assert_eq!(timeline.stats.peak_dbfs, Some(f32::NEG_INFINITY));
    assert!(timeline.render(48000, 1.0).is_ok());
    assert!(timeline.render(1, 1.0).unwrap_err().contains("one second"));
}
#[test]
fn absolute_sample_placement_preserves_drift_and_rejects_excess_backward_steps() {
    let mut timeline = AudioTimeline::new(0);
    // +1000 ppm: each 100 ms source buffer advances by 100.1 ms.
    for i in 0..10 {
        timeline.ingest(block(i * 100_100_000, 4800, 0.25)).unwrap();
    }
    let pcm = timeline.render(48000, 1.0).unwrap();
    assert_eq!(pcm.len(), 96000);
    assert!((42..=44).contains(&timeline.stats.silence_samples));
    assert_eq!(timeline.stats.max_timestamp_step_samples, 5);
    let mut timeline = AudioTimeline::new(0);
    timeline.ingest(block(0, 480, 0.5)).unwrap();
    assert!(timeline
        .ingest(block(1_000_000, 480, 0.5))
        .unwrap_err()
        .contains("5 ms"));
}
#[test]
fn alignment_memory_and_arithmetic_are_bounded() {
    let mut timeline = AudioTimeline::new(u64::MAX - 100);
    timeline.ingest(block(0, 480, 0.5)).unwrap(); // entirely before T0
    assert_eq!(timeline.render(1, 1.0).unwrap(), vec![0.0; 2]);
    assert!(timeline.render(48001, 1.0).is_err());
    assert!(timeline.render(1, f32::NAN).is_err());
    assert!(timeline.ingest(block(0, 4801, 0.5)).is_err());
    let mut timeline = AudioTimeline::new(0);
    for i in 0..128 {
        timeline.ingest(block(i * 10_000_000, 480, 0.5)).unwrap();
    }
    assert!(timeline
        .ingest(block(1_280_000_000, 480, 0.5))
        .unwrap_err()
        .contains("full"));
}
#[test]
fn live_gstreamer_buffers_share_video_epoch_and_have_real_levels() {
    let before = capture_clock_now_ns().unwrap();
    let mut input = AudioInput::start(AudioSource::synthetic()).unwrap();
    let first = input.receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(first.pts_ns >= before);
    assert_eq!(first.pcm.len(), 960);
    let (peak, rms) = levels(&first.pcm);
    assert!((peak + 6.0206).abs() < 0.1, "{peak}");
    assert!((rms + 9.0309).abs() < 0.2, "{rms}");
    let second = input.receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(second.pts_ns - first.pts_ns, 10_000_000);
    assert!(matches!(input.status().state, AudioState::Capturing));
    let start = Instant::now();
    input.stop();
    assert!(start.elapsed() < Duration::from_secs(1));
    assert_eq!(input.status().state, AudioState::Stopped);
}
#[test]
fn overflow_and_terminal_errors_remain_observable_without_draining() {
    let mut input = AudioInput::start(AudioSource::synthetic()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while input.status().queue_drops == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(input.status().queue_drops > 0);
    input.stop();
    assert!(input.receiver.try_iter().count() <= 16);
    let input = AudioInput::start(AudioSource {
        factory: "nonexistent_audio_source".into(),
        device: None,
    })
    .unwrap();
    assert!(input.receiver.recv_timeout(Duration::from_secs(5)).is_err());
    assert!(matches!(input.status().state, AudioState::Error(_)));
    let mut timeline = AudioTimeline::new(0);
    assert!(timeline.collect(&input, 960, 1.0).is_err());
}

#[cfg(feature = "gstreamer-recording")]
#[test]
fn independent_live_video_audio_to_program_webm_with_delayed_audio_and_trailing_video_gaps() {
    use gst::prelude::*;
    use gstreamer as gst;
    use gstreamer_app as gst_app;
    use openvideomix_core::video_profile::VideoProfile;
    use openvideomix_desktop::{
        capture::{CameraEvent, CameraSession},
        capture_output::CaptureOutput,
        program_recording::ProgramRecorder,
    };
    let file = std::env::temp_dir().join(format!("live-av-{}.webm", std::process::id()));
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Cleanup(file);
    let profile = VideoProfile {
        width: 640,
        height: 360,
        frame_rate: openvideomix_core::camera::FrameRate::new(30, 1).unwrap(),
    };
    // Explicit reduced development profile; does not qualify 1080p50.
    let camera = CameraSession::start_synthetic();
    let first = match camera
        .receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
    {
        CameraEvent::Frame(f) => f,
        CameraEvent::Error(e) => panic!("{e}"),
    };
    let t0 = first.capture_pts_ns.unwrap();
    let mut output = CaptureOutput::new(profile).unwrap();
    output.add_source(1, 4).unwrap();
    output.ingest(1, first).unwrap();
    std::thread::sleep(Duration::from_millis(60)); // independent, deliberately delayed source
    let audio = AudioInput::start(AudioSource::synthetic()).unwrap();
    let mut timeline = AudioTimeline::new(t0);
    let mut recorder = ProgramRecorder::start(&file.0, profile).unwrap();
    for index in 0..18 {
        // Pace using the existing capture clock; never synthesize source PCM.
        let target = t0 + (index + 1) * 1_000_000_000 / 30;
        while capture_clock_now_ns().unwrap() < target {
            std::thread::sleep(Duration::from_millis(1));
        }
        for event in camera.receiver.try_iter() {
            match event {
                CameraEvent::Frame(f) => {
                    let result = output.ingest(1, f);
                    if let Err(error) = result {
                        assert_eq!(
                            error,
                            openvideomix_desktop::capture_output::IngressError::Output(
                                openvideomix_core::reference_output::RenderError::Queue(
                                    openvideomix_core::reference_output::FrameQueueError::LateInput
                                )
                            )
                        );
                    }
                }
                CameraEvent::Error(e) => panic!("{e}"),
            }
        }
        let frame = output
            .render_at(t0 + index * 1_000_000_000 / 30, 1, None, [None; 4])
            .unwrap();
        let count = recorder.required_audio_samples_for_frame(index).unwrap();
        let pcm = timeline.collect(&audio, count, 0.5).unwrap();
        recorder
            .wait_for_frame_capacity(index, Duration::from_secs(2))
            .unwrap();
        let missing = matches!(index, 4 | 5 | 16 | 17);
        recorder
            .write_slot_at(index, if missing { None } else { Some(&frame) }, &pcm)
            .unwrap();
    }
    let stats = recorder.finish(Duration::from_secs(10)).unwrap();
    assert_eq!(
        (
            stats.submitted_video_frames,
            stats.timeline_video_frames,
            stats.missing_video_frames
        ),
        (14, 18, 4)
    );
    assert_eq!(stats.submitted_audio_samples_per_channel, 28800);
    assert!(
        (2000..12000).contains(&timeline.stats.silence_samples),
        "{:?}",
        timeline.stats
    );
    let decoder=gst::parse::launch("filesrc name=input ! matroskademux name=d d.video_0 ! queue ! vp8dec ! appsink name=v sync=false wait-on-eos=false d.audio_0 ! queue ! vorbisdec ! audioconvert ! audio/x-raw,format=F32LE ! appsink name=a sync=false wait-on-eos=false").unwrap().dynamic_cast::<gst::Pipeline>().unwrap();
    struct Stop(gst::Pipeline);
    impl Drop for Stop {
        fn drop(&mut self) {
            let _ = self.0.set_state(gst::State::Null);
        }
    }
    let decoder = Stop(decoder);
    decoder
        .0
        .by_name("input")
        .unwrap()
        .set_property("location", file.0.to_str().unwrap());
    let v = decoder
        .0
        .by_name("v")
        .unwrap()
        .dynamic_cast::<gst_app::AppSink>()
        .unwrap();
    let a = decoder
        .0
        .by_name("a")
        .unwrap()
        .dynamic_cast::<gst_app::AppSink>()
        .unwrap();
    decoder.0.set_state(gst::State::Playing).unwrap();
    let message = decoder
        .0
        .bus()
        .unwrap()
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(15),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .unwrap();
    assert!(
        matches!(message.view(), gst::MessageView::Eos(_)),
        "{message:?}"
    );
    let mut video_pts = Vec::new();
    while let Some(sample) = v.try_pull_sample(gst::ClockTime::ZERO) {
        video_pts.push(sample.buffer().unwrap().pts().unwrap().nseconds());
    }
    assert_eq!(video_pts.len(), 14);
    assert!(video_pts.windows(2).all(|p| p[1] > p[0]));
    assert!((199_000_000..=201_000_000).contains(&video_pts[4]));
    let mut decoded = Vec::new();
    let mut previous = None;
    let mut end = 0;
    while let Some(sample) = a.try_pull_sample(gst::ClockTime::ZERO) {
        let caps = sample.caps().unwrap().structure(0).unwrap();
        assert_eq!(caps.get::<i32>("rate").unwrap(), 48000);
        assert_eq!(caps.get::<i32>("channels").unwrap(), 2);
        let b = sample.buffer().unwrap();
        let pts = b.pts().unwrap().nseconds();
        if let Some(last) = previous {
            assert!(pts >= last);
        }
        previous = Some(pts);
        end = pts + b.duration().unwrap().nseconds();
        decoded.extend(
            b.map_readable()
                .unwrap()
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b)),
        );
    }
    assert!(
        (57600..=59648).contains(&decoded.len()),
        "samples {}",
        decoded.len()
    );
    assert!(
        (599_000_000..=622_000_000).contains(&end),
        "audio end {end}"
    );
    let (peak, rms) = levels(&decoded);
    assert!((-13.0..-10.0).contains(&peak), "peak {peak}");
    assert!((-20.0..-14.0).contains(&rms), "rms {rms}");
    assert!(levels(&decoded[..1920]).0 < -40.0, "startup offset lost");
    eprintln!("synthetic live AV: {stats:?}; alignment {:?}; decoded end {end} ns, peak {peak}, rms {rms} dBFS",timeline.stats);
}

#[test]
fn negative_drift_trims_overlaps_without_overwriting_accepted_samples() {
    let mut timeline = AudioTimeline::new(0);
    timeline.ingest(block(0, 4800, 0.25)).unwrap();
    timeline.ingest(block(99_900_000, 4800, 0.5)).unwrap();
    let pcm = timeline.render(9595, 1.0).unwrap();
    assert!(pcm[..9600].iter().all(|v| *v == 0.25));
    assert!(pcm[9600..].iter().all(|v| *v == 0.5));
    assert_eq!(timeline.stats.overlap_samples, 5);
    assert_eq!(timeline.stats.silence_samples, 0);
}

#[cfg(feature = "gstreamer-recording")]
#[test]
fn desktop_service_explicit_live_selection_records_default_1080p50() {
    use openvideomix_desktop::{
        capture::CameraSession,
        desktop_production::{DesktopProduction, RecordingState},
    };
    let studio = DesktopProduction::start(&[1]).unwrap();
    let _camera = CameraSession::start_production_test(1, 1, studio.ingress());
    let deadline = Instant::now() + Duration::from_secs(15);
    while studio.view().latest.is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(studio.view().latest.is_some());
    let directory = std::env::temp_dir();
    let path = directory.join(format!("live-desktop-{}.webm", std::process::id()));
    studio.approve_recording_directory(&directory).unwrap();
    studio
        .start_live_recording(path.clone(), AudioSource::synthetic())
        .unwrap();
    loop {
        let view = studio.view();
        match view.recording {
            RecordingState::Error(error) => panic!("{error}"),
            RecordingState::Recording { submitted, .. }
                if submitted >= 10
                    && view
                        .audio_alignment
                        .peak_dbfs
                        .is_some_and(|peak| peak > -30.0) =>
            {
                break
            }
            _ => assert!(
                Instant::now() < deadline,
                "no live recording: {:?}",
                view.recording
            ),
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    studio.stop_recording().unwrap();
    loop {
        let view = studio.view();
        match view.recording {
            RecordingState::Saved(stats) => {
                assert!(stats.submitted_video_frames >= 4);
                assert_eq!(
                    stats.submitted_audio_samples_per_channel,
                    stats.timeline_video_frames * 960
                );
                assert_eq!(view.audio_label, "Synthetic live GStreamer audio");
                let audio = view.audio_status.unwrap();
                assert!(audio.buffers > 0);
                assert_eq!(audio.state, AudioState::Stopped);
                assert!(view.audio_alignment.peak_dbfs.unwrap() > -30.0);
                eprintln!(
                    "synthetic default-profile desktop service: {stats:?}; {:?}",
                    view.audio_alignment
                );
                break;
            }
            RecordingState::Error(error) => panic!("{error}"),
            _ => assert!(Instant::now() < deadline, "finalization timeout"),
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(studio);
    std::fs::remove_file(path).unwrap();
}
