#![cfg(feature = "gstreamer-capture")]
use openvideomix_core::{
    camera::FrameRate,
    reference_compositor::CompositeError,
    reference_output::{FrameQueueError, RenderError},
    video_profile::VideoProfile,
};
use openvideomix_desktop::{
    capture::{CameraEvent, CameraFrame, CameraSession},
    capture_output::{CaptureOutput, IngressError},
};
use std::time::Duration;

fn profile() -> VideoProfile {
    VideoProfile {
        width: 2,
        height: 1,
        ..VideoProfile::default()
    }
}
fn frame(shared: Option<u64>, red: u8) -> CameraFrame {
    CameraFrame {
        width: 2,
        height: 1,
        rgba: vec![red, 0, 0, 255, red, 0, 0, 255],
        pts_ns: Some(0),
        capture_pts_ns: shared,
        duration_ns: None,
    }
}

#[test]
fn delayed_sources_use_common_pts_and_never_leak_future_frames() {
    let mut output = CaptureOutput::new(profile()).unwrap();
    output.add_source(1, 3).unwrap();
    output.add_source(2, 3).unwrap();
    output.ingest(1, frame(Some(100), 10)).unwrap();
    output.ingest(1, frame(Some(120), 20)).unwrap();
    output.ingest(2, frame(Some(150), 90)).unwrap();
    // Both sources have raw local PTS zero. Source 2 must still be unavailable.
    assert_eq!(
        output.render_at(110, 2, None, [None; 4]),
        Err(IngressError::Output(RenderError::UnavailableFrame(2)))
    );
    assert_eq!(
        output.render_at(110, 1, None, [None; 4]).unwrap().pixels[0],
        10
    );
    assert_eq!(
        output.render_at(140, 1, None, [None; 4]).unwrap().pixels[0],
        20
    );
    assert_eq!(
        output.render_at(150, 2, None, [None; 4]).unwrap().pixels[0],
        90
    );
    assert_eq!(output.stats(2).unwrap().last_source_pts_ns, Some(0));
    assert_eq!(output.stats(2).unwrap().last_capture_pts_ns, Some(150));
}

#[test]
fn validates_inputs_without_losing_existing_queue_and_reports_real_overflow() {
    let mut output = CaptureOutput::new(profile()).unwrap();
    assert_eq!(
        output.ingest(1, frame(Some(0), 1)),
        Err(IngressError::UnknownInput(1))
    );
    assert_eq!(
        output.add_source(1, 0),
        Err(IngressError::Queue(FrameQueueError::InvalidCapacity))
    );
    output.add_source(1, 1).unwrap();
    output.ingest(1, frame(Some(10), 1)).unwrap();
    assert_eq!(
        output.add_source(1, 1),
        Err(IngressError::DuplicateInput(1))
    );
    assert_eq!(
        output.ingest(1, frame(None, 9)),
        Err(IngressError::MissingSharedTimestamp)
    );
    let mut bad = frame(Some(20), 9);
    bad.width = 3;
    assert_eq!(output.ingest(1, bad), Err(IngressError::DimensionMismatch));
    let mut bad = frame(Some(20), 9);
    bad.rgba.pop();
    assert_eq!(
        output.ingest(1, bad),
        Err(IngressError::InvalidPixels(CompositeError::BufferSize))
    );
    assert_eq!(
        output.render_at(10, 1, None, [None; 4]).unwrap().pixels[0],
        1
    );
    assert_eq!(
        output.ingest(1, frame(Some(10), 8)),
        Err(IngressError::Output(RenderError::Queue(
            FrameQueueError::LateInput
        )))
    );
    output.ingest(1, frame(Some(20), 2)).unwrap();
    output.ingest(1, frame(Some(30), 3)).unwrap();
    let stats = output.stats(1).unwrap();
    assert_eq!(
        (
            stats.accepted_frames,
            stats.rejected_frames,
            stats.queue_overflow_frames
        ),
        (3, 4, 1)
    );
    assert_eq!(
        output.render_at(30, 1, None, [None; 4]).unwrap().pixels[0],
        3
    );
    output.remove_source(1);
    assert!(output.stats(1).is_none());
    assert_eq!(
        output.render_at(40, 1, None, [None; 4]),
        Err(IngressError::Output(RenderError::UnknownInput(1)))
    );
}

#[test]
fn default_profile_is_preserved_and_small_capture_requires_explicit_alternative() {
    let mut default = CaptureOutput::default();
    assert_eq!(default.profile(), VideoProfile::default());
    default.add_source(1, 1).unwrap();
    assert_eq!(
        default.ingest(1, frame(Some(0), 1)),
        Err(IngressError::DimensionMismatch)
    );
    assert_eq!(default.profile().frame_rate, FrameRate::PAL_50);
    assert!(matches!(
        CaptureOutput::new(VideoProfile {
            width: 0,
            ..profile()
        }),
        Err(IngressError::InvalidProfile)
    ));
}

#[test]
fn actual_synthetic_capture_reaches_headless_program_without_gui() {
    let session = CameraSession::start_synthetic();
    let frame = match session
        .receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
    {
        CameraEvent::Frame(frame) => frame,
        CameraEvent::Error(error) => panic!("{error}"),
    };
    let deadline = frame.capture_pts_ns.unwrap();
    let expected = frame.rgba.clone();
    // Explicitly opt into the existing experimental source mode; never change default.
    let profile = VideoProfile {
        width: 640,
        height: 360,
        frame_rate: FrameRate::new(30, 1).unwrap(),
    };
    let mut output = CaptureOutput::new(profile).unwrap();
    output.add_source(1, 2).unwrap();
    output.ingest(1, frame).unwrap();
    assert_eq!(
        output
            .render_at(deadline, 1, None, [None; 4])
            .unwrap()
            .pixels,
        expected
    );
}

#[cfg(feature = "gstreamer-recording")]
#[test]
fn captured_program_records_and_decodes_with_supplied_pcm() {
    use gst::prelude::*;
    use gstreamer as gst;
    use gstreamer_app as gst_app;
    use openvideomix_desktop::program_recording::ProgramRecorder;
    let path = std::env::temp_dir().join(format!(
        "openvideomix-capture-output-{}.webm",
        std::process::id()
    ));
    struct File(std::path::PathBuf);
    impl Drop for File {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = File(path);
    let profile = VideoProfile {
        width: 640,
        height: 360,
        frame_rate: FrameRate::new(30, 1).unwrap(),
    };
    let session = CameraSession::start_synthetic();
    let mut output = CaptureOutput::new(profile).unwrap();
    output.add_source(1, 2).unwrap();
    let mut recorder = ProgramRecorder::start(&file.0, profile).unwrap();
    for _ in 0..3 {
        let frame = match session
            .receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
        {
            CameraEvent::Frame(frame) => frame,
            CameraEvent::Error(error) => panic!("{error}"),
        };
        let deadline = frame.capture_pts_ns.unwrap();
        output.ingest(1, frame).unwrap();
        let program = output.render_at(deadline, 1, None, [None; 4]).unwrap();
        let samples = recorder.next_audio_samples_per_channel().unwrap();
        recorder
            .write_frame(&program, &vec![0.1; samples as usize * 2])
            .unwrap();
    }
    let stats = recorder.finish(Duration::from_secs(10)).unwrap();
    assert_eq!(stats.encoded_video_frames, 3);
    assert_eq!(stats.submitted_audio_samples_per_channel, 4800);
    assert_eq!(output.stats(1).unwrap().accepted_frames, 3);
    let pipeline=gst::parse::launch("filesrc name=input ! matroskademux name=d d.video_0 ! queue ! vp8dec ! videoconvert ! video/x-raw,format=RGBA ! appsink name=decoded sync=false wait-on-eos=false d.audio_0 ! queue ! vorbisdec ! fakesink sync=false").unwrap().dynamic_cast::<gst::Pipeline>().unwrap();
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
        .set_property("location", file.0.to_str().unwrap());
    let sink = decoder
        .0
        .by_name("decoded")
        .unwrap()
        .dynamic_cast::<gst_app::AppSink>()
        .unwrap();
    decoder.0.set_state(gst::State::Playing).unwrap();
    let message = decoder
        .0
        .bus()
        .unwrap()
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(10),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .unwrap();
    assert!(
        matches!(message.view(), gst::MessageView::Eos(_)),
        "Decode error: {message:?}"
    );
    let mut decoded = 0;
    while let Some(sample) = sink.try_pull_sample(gst::ClockTime::ZERO) {
        let caps = sample.caps().unwrap().structure(0).unwrap();
        assert_eq!(caps.get::<i32>("width").unwrap(), 640);
        assert_eq!(caps.get::<i32>("height").unwrap(), 360);
        assert_eq!(
            caps.get::<gst::Fraction>("framerate").unwrap(),
            gst::Fraction::new(30, 1)
        );
        decoded += 1;
    }
    assert_eq!(decoded, 3);
}
