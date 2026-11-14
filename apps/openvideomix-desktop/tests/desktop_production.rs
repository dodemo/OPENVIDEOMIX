#![cfg(all(feature = "gstreamer-capture", feature = "gstreamer-recording"))]
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use openvideomix_core::{
    commands::{CommandDispatcher, ProductionCommand},
    InputKind, MixerState, VideoInput,
};
use openvideomix_desktop::{
    capture::CameraSession,
    desktop_production::{DesktopProduction, RecordingState},
};
use std::{
    thread,
    time::{Duration, Instant},
};
#[track_caller]
fn wait_until(studio: &DesktopProduction, mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if predicate() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "Desktop condition timeout: recording={:?}, error={:?}, stats={:?}",
        studio.view().recording,
        studio.view().program_error,
        studio.view().stats
    );
}
#[test]
fn capture_cut_program_and_recording_work_without_gui_polling() {
    let studio = DesktopProduction::start(&[41, 42, 43, 44]).unwrap();
    // Camera GUI queues are deliberately never drained in this test.
    let _first = CameraSession::start_production_test(41, 0, studio.ingress());
    let _second = CameraSession::start_production_test(42, 3, studio.ingress());
    let _third = CameraSession::start_production_test(43, 1, studio.ingress());
    let _fourth = CameraSession::start_production_test(44, 2, studio.ingress());
    let mut state = MixerState::default();
    let mut dispatcher = CommandDispatcher::default();
    for id in [41, 42] {
        state.inputs.push(VideoInput::new(
            id,
            "test",
            InputKind::Camera {
                device_path: "test".into(),
            },
        ));
    }
    studio.route(state.program_input, None, 0.8, false);
    dispatcher
        .dispatch(&mut state, ProductionCommand::SelectPreview { input: 41 })
        .unwrap();
    dispatcher
        .dispatch(&mut state, ProductionCommand::Cut)
        .unwrap();
    studio.route(state.program_input, None, 0.8, false);
    wait_until(&studio, || studio.view().latest.is_some());
    let first_pixel = studio.view().latest.unwrap().1.pixels[..4].to_vec();
    let sustained = Instant::now() + Duration::from_secs(2);
    let before = studio.view().stats.rendered_frames;
    while Instant::now() < sustained {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        studio.view().latest.is_some(),
        "The live source must not age out after a short successful startup"
    );
    assert!(studio.view().stats.rendered_frames > before + 20);

    let directory =
        std::env::temp_dir().join(format!("openvideomix-desktop-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    struct Clean(std::path::PathBuf);
    impl Drop for Clean {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let directory = Clean(directory);
    let path = directory.0.join("program.webm");
    assert!(
        studio.start_test_recording(path.clone()).is_err(),
        "Control intent alone cannot approve a destination directory"
    );
    studio.approve_recording_directory(&directory.0).unwrap();
    studio.start_test_recording(path.clone()).unwrap();
    wait_until(
        &studio,
        || matches!(studio.view().recording,RecordingState::Recording {submitted,..} if submitted>=5),
    );
    dispatcher
        .dispatch(&mut state, ProductionCommand::SelectPreview { input: 42 })
        .unwrap();
    dispatcher
        .dispatch(&mut state, ProductionCommand::Cut)
        .unwrap();
    assert_eq!(state.preview_input, Some(41));
    studio.route(state.program_input, None, 0.8, false);
    wait_until(&studio, || {
        studio
            .view()
            .latest
            .is_some_and(|(_, frame)| frame.pixels[..4] != first_pixel)
    });
    wait_until(
        &studio,
        || matches!(studio.view().recording,RecordingState::Recording {submitted,..} if submitted>=12),
    );
    studio.stop_recording().unwrap();
    wait_until(&studio, || {
        matches!(
            studio.view().recording,
            RecordingState::Saved(_) | RecordingState::Error(_)
        )
    });
    let stats = match studio.view().recording {
        RecordingState::Saved(stats) => stats,
        other => panic!("Recording did not finish: {other:?}"),
    };
    assert!(stats.encoded_video_frames >= 12);
    assert_eq!(stats.encoded_video_frames, stats.submitted_video_frames);
    assert!(stats.submitted_audio_samples_per_channel >= 12 * 960);
    let pipeline=gst::parse::launch("filesrc name=input ! matroskademux name=d d.video_0 ! queue ! vp8dec ! videoconvert ! video/x-raw,format=RGBA ! appsink name=video sync=false wait-on-eos=false d.audio_0 ! queue ! vorbisdec ! audioconvert ! audio/x-raw,format=F32LE ! appsink name=audio sync=false wait-on-eos=false").unwrap().dynamic_cast::<gst::Pipeline>().unwrap();
    struct Stop(gst::Pipeline);
    impl Drop for Stop {
        fn drop(&mut self) {
            let _ = self.0.set_state(gst::State::Null);
        }
    }
    let decoder = Stop(pipeline);
    decoder
        .0
        .by_name("input")
        .unwrap()
        .set_property("location", path.to_str().unwrap());
    let video = decoder
        .0
        .by_name("video")
        .unwrap()
        .dynamic_cast::<gst_app::AppSink>()
        .unwrap();
    let audio = decoder
        .0
        .by_name("audio")
        .unwrap()
        .dynamic_cast::<gst_app::AppSink>()
        .unwrap();
    decoder.0.set_state(gst::State::Playing).unwrap();
    let msg = decoder
        .0
        .bus()
        .unwrap()
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(15),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .unwrap();
    assert!(matches!(msg.view(), gst::MessageView::Eos(_)), "{msg:?}");
    let mut pixels = Vec::new();
    let mut count = 0;
    while let Some(sample) = video.try_pull_sample(gst::ClockTime::ZERO) {
        let caps = sample.caps().unwrap().structure(0).unwrap();
        assert_eq!(caps.get::<i32>("width").unwrap(), 1920);
        assert_eq!(caps.get::<i32>("height").unwrap(), 1080);
        assert_eq!(
            caps.get::<gst::Fraction>("framerate").unwrap(),
            gst::Fraction::new(50, 1)
        );
        pixels.push(sample.buffer().unwrap().map_readable().unwrap()[..4].to_vec());
        count += 1;
    }
    assert_eq!(count, stats.encoded_video_frames);
    assert_ne!(
        pixels.first(),
        pixels.last(),
        "CUT must change actual recorded Program pixels"
    );
    let mut peak = 0.0_f32;
    let mut samples = 0;
    while let Some(sample) = audio.try_pull_sample(gst::ClockTime::ZERO) {
        let caps = sample.caps().unwrap().structure(0).unwrap();
        assert_eq!(caps.get::<i32>("rate").unwrap(), 48000);
        for bytes in sample
            .buffer()
            .unwrap()
            .map_readable()
            .unwrap()
            .as_chunks::<4>()
            .0
        {
            peak = peak.max(f32::from_le_bytes(*bytes).abs());
            samples += 1;
        }
    }
    assert!(peak > 0.2 && peak < 0.6);
    assert!(samples > 0);
}
