#![cfg(feature = "gstreamer-recording")]

use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use openvideomix_core::{
    camera::FrameRate,
    media_timebase::video_frame_pts_ns,
    reference_compositor::{RgbaFrame, Transition},
    reference_output::{ReferenceOutput, ReferenceTake, TimedFrame},
    video_profile::VideoProfile,
};
use openvideomix_desktop::program_recording::ProgramRecorder;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

struct TestFile(PathBuf);
impl TestFile {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(std::env::temp_dir().join(format!(
            "openvideomix-{}-{}.webm",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for TestFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn small_profile() -> VideoProfile {
    VideoProfile {
        width: 64,
        height: 32,
        ..VideoProfile::default()
    }
}

#[test]
fn refuses_existing_files_and_invalid_profiles() {
    let file = TestFile::new();
    std::fs::write(&file.0, b"preserve original").unwrap();
    assert!(ProgramRecorder::start(&file.0, small_profile()).is_err());
    assert_eq!(std::fs::read(&file.0).unwrap(), b"preserve original");
    let invalid = VideoProfile {
        width: 0,
        ..small_profile()
    };
    assert!(ProgramRecorder::start(&file.0, invalid).is_err());
    assert!(ProgramRecorder::start(&file.0.join("missing/output.webm"), small_profile()).is_err());
}

#[test]
fn malformed_input_is_rejected_without_advancing_audio_video() {
    let file = TestFile::new();
    let mut recorder = ProgramRecorder::start(&file.0, small_profile()).unwrap();
    let valid = RgbaFrame::solid(64, 32, [255, 0, 0, 255]).unwrap();
    let invalid = RgbaFrame::solid(1, 1, [0, 0, 0, 255]).unwrap();
    let mut audio = vec![0.1; 960 * 2];
    assert!(recorder.write_frame(&invalid, &audio).is_err());
    assert!(recorder.write_frame(&valid, &audio[..4]).is_err());
    audio[0] = f32::NAN;
    assert!(recorder.write_frame(&valid, &audio).is_err());
    assert_eq!(recorder.next_audio_samples_per_channel().unwrap(), 960);
    audio[0] = 0.1;
    recorder.write_frame(&valid, &audio).unwrap();
    let stats = recorder.finish(Duration::from_secs(10)).unwrap();
    assert_eq!(stats.submitted_video_frames, 1);
    assert_eq!(stats.encoded_video_frames, 1);
    assert_eq!(stats.submitted_audio_samples_per_channel, 960);
}

#[test]
fn fractional_rate_audio_sample_counts_do_not_accumulate_rounding_error() {
    let file = TestFile::new();
    let profile = VideoProfile {
        frame_rate: FrameRate::new(60_000, 1001).unwrap(),
        ..small_profile()
    };
    let mut recorder = ProgramRecorder::start(&file.0, profile).unwrap();
    let video = RgbaFrame::solid(64, 32, [0, 0, 0, 255]).unwrap();
    for _ in 0..5 {
        let count = recorder.next_audio_samples_per_channel().unwrap();
        recorder
            .write_frame(&video, &vec![0.1; count as usize * 2])
            .unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }
    let stats = recorder.finish(Duration::from_secs(10)).unwrap();
    assert_eq!(stats.submitted_audio_samples_per_channel, 4004);
    assert_eq!(stats.encoded_video_frames, 5);
}

#[test]
fn empty_recording_is_never_reported_as_saved() {
    let file = TestFile::new();
    let recorder = ProgramRecorder::start(&file.0, small_profile()).unwrap();
    assert!(recorder.finish(Duration::from_secs(1)).is_err());
}

#[test]
fn actual_composited_program_and_pcm_round_trip_at_default_1080p50() {
    let file = TestFile::new();
    let profile = VideoProfile::default();
    let mut recorder = ProgramRecorder::start(&file.0, profile).unwrap();
    let mut program = ReferenceOutput::default();
    for (id, color) in [
        (1, [255, 0, 0, 255]),
        (2, [0, 0, 255, 255]),
        (3, [0, 255, 0, 128]),
    ] {
        program.add_source(id, 1).unwrap();
        program
            .push(
                id,
                TimedFrame {
                    pts_ns: 0,
                    rgba: RgbaFrame::solid(profile.width, profile.height, color).unwrap(),
                },
            )
            .unwrap();
    }
    let mut sample_index = 0;
    for index in 0..10 {
        let pts = video_frame_pts_ns(index, profile.frame_rate).unwrap();
        let composed = program
            .render_at(
                pts,
                1,
                Some(ReferenceTake {
                    destination: 2,
                    transition: Transition::Fade,
                    progress: index as f32 / 9.0,
                }),
                [Some(3), None, None, None],
            )
            .unwrap();
        let count = recorder.next_audio_samples_per_channel().unwrap();
        let mut pcm = Vec::with_capacity(count as usize * 2);
        for _ in 0..count {
            let tone =
                (sample_index as f32 * std::f32::consts::TAU * 1000.0 / 48_000.0).sin() * 0.5;
            pcm.extend([tone, tone]);
            sample_index += 1;
        }
        recorder.write_frame(&composed, &pcm).unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }
    let stats = recorder.finish(Duration::from_secs(20)).unwrap();
    assert_eq!(stats.submitted_video_frames, 10);
    assert_eq!(stats.encoded_video_frames, 10);
    assert_eq!(stats.submitted_audio_samples_per_channel, 9600);
    assert!(std::fs::metadata(&file.0).unwrap().len() > 1000);

    // Decode the finished file, proving playable streams, actual pixel content
    // and actual non-silent PCM. This is synthetic validation, not hardware I/O.
    let pipeline = gst::parse::launch(
        "filesrc name=input ! matroskademux name=demux \
         demux.video_0 ! queue ! vp8dec ! videoconvert ! video/x-raw,format=RGBA ! \
         appsink name=video sync=false wait-on-eos=false \
         demux.audio_0 ! queue ! vorbisdec ! audioconvert ! audio/x-raw,format=F32LE ! \
         appsink name=audio sync=false wait-on-eos=false",
    )
    .unwrap()
    .dynamic_cast::<gst::Pipeline>()
    .unwrap();
    struct Stop(gst::Pipeline);
    impl Drop for Stop {
        fn drop(&mut self) {
            let _ = self.0.set_state(gst::State::Null);
        }
    }
    let pipeline = Stop(pipeline);
    pipeline
        .0
        .by_name("input")
        .unwrap()
        .set_property("location", file.0.to_str().unwrap());
    let video = pipeline
        .0
        .by_name("video")
        .unwrap()
        .dynamic_cast::<gst_app::AppSink>()
        .unwrap();
    let audio = pipeline
        .0
        .by_name("audio")
        .unwrap()
        .dynamic_cast::<gst_app::AppSink>()
        .unwrap();
    pipeline.0.set_state(gst::State::Playing).unwrap();
    let message = pipeline
        .0
        .bus()
        .unwrap()
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(20),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .unwrap();
    assert!(
        matches!(message.view(), gst::MessageView::Eos(_)),
        "Decode failed: {message:?}"
    );
    let mut colors = Vec::new();
    let mut previous_pts = None;
    while let Some(sample) = video.try_pull_sample(gst::ClockTime::ZERO) {
        let caps = sample.caps().unwrap().structure(0).unwrap();
        assert_eq!(caps.get::<i32>("width").unwrap(), 1920);
        assert_eq!(caps.get::<i32>("height").unwrap(), 1080);
        assert_eq!(
            caps.get::<gst::Fraction>("framerate").unwrap(),
            gst::Fraction::new(50, 1)
        );
        let buffer = sample.buffer().unwrap();
        let pts = buffer.pts().unwrap().nseconds();
        if let Some(previous) = previous_pts {
            assert_eq!(pts - previous, 20_000_000);
        }
        previous_pts = Some(pts);
        colors.push(buffer.map_readable().unwrap()[..4].to_vec());
    }
    assert_eq!(colors.len(), 10);
    assert!(
        colors[0][0] > 95 && colors[0][1] > 95 && colors[0][2] < 32,
        "First Program pixel: {:?}",
        colors[0]
    );
    assert!(
        colors[9][0] < 32 && colors[9][1] > 95 && colors[9][2] > 95,
        "Last Program pixel: {:?}",
        colors[9]
    );
    let mut decoded_samples = 0;
    let mut peak = 0.0_f32;
    let mut squared = 0.0_f64;
    while let Some(sample) = audio.try_pull_sample(gst::ClockTime::ZERO) {
        let caps = sample.caps().unwrap().structure(0).unwrap();
        assert_eq!(caps.get::<i32>("rate").unwrap(), 48_000);
        assert_eq!(caps.get::<i32>("channels").unwrap(), 2);
        let buffer = sample.buffer().unwrap().map_readable().unwrap();
        for bytes in buffer.as_chunks::<4>().0 {
            let value = f32::from_le_bytes(*bytes);
            assert!(value.is_finite());
            peak = peak.max(value.abs());
            squared += f64::from(value).powi(2);
            decoded_samples += 1;
        }
    }
    // Vorbis may add a final short padded block; allow at most one codec block.
    assert!(
        (19_200..=21_248).contains(&decoded_samples),
        "PCM samples: {decoded_samples}"
    );
    assert!(peak > 0.4 && peak < 0.7, "Decoded PCM peak: {peak}");
    let rms = (squared / decoded_samples as f64).sqrt();
    assert!((0.30..0.40).contains(&rms), "Decoded PCM RMS: {rms}");
}
