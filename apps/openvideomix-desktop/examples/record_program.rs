//! Deterministic CPU Program -> WebM proof of concept, with generated PCM.
//! Run with --features gstreamer-recording --example record_program -- file.webm
use openvideomix_core::{
    media_timebase::video_frame_pts_ns,
    reference_compositor::{RgbaFrame, Transition},
    reference_output::{ReferenceOutput, ReferenceTake, TimedFrame},
    video_profile::{VideoProfile, DEFAULT_AUDIO_SAMPLE_RATE},
};
use openvideomix_desktop::program_recording::ProgramRecorder;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Usage: record_program OUTPUT.webm")?,
    );
    let gapped = match std::env::args().nth(2).as_deref() {
        None => false,
        Some("--gapped") => true,
        _ => return Err("Usage: record_program OUTPUT.webm [--gapped]".into()),
    };
    let profile = VideoProfile::default();
    let mut program = ReferenceOutput::default();
    let red = RgbaFrame::solid(profile.width, profile.height, [220, 10, 10, 255])
        .map_err(|e| format!("RGBA: {e:?}"))?;
    let blue = RgbaFrame::solid(profile.width, profile.height, [10, 10, 220, 255])
        .map_err(|e| format!("RGBA: {e:?}"))?;
    let mut overlay = RgbaFrame::solid(profile.width, profile.height, [0, 0, 0, 0])
        .map_err(|e| format!("RGBA: {e:?}"))?;
    for y in 880..1000 {
        for x in 120..1800 {
            let offset = ((y * profile.width + x) * 4) as usize;
            overlay.pixels[offset..offset + 4].copy_from_slice(&[10, 220, 10, 180]);
        }
    }
    for (id, rgba) in [(1, red), (2, blue), (3, overlay)] {
        program
            .add_source(id, 1)
            .map_err(|e| format!("Source queue: {e:?}"))?;
        program
            .push(id, TimedFrame { pts_ns: 0, rgba })
            .map_err(|e| format!("Source frame: {e:?}"))?;
    }
    let mut recorder = ProgramRecorder::start(&path, profile)?;
    let started = Instant::now();
    let mut sample_index = 0;
    for index in 0..50 {
        // Omit ten video slots explicitly, while supplying uninterrupted PCM.
        if gapped && (10..20).contains(&index) {
            continue;
        }
        let composed = program
            .render_at(
                video_frame_pts_ns(index, profile.frame_rate).ok_or("Invalid PTS")?,
                1,
                Some(ReferenceTake {
                    destination: 2,
                    transition: Transition::Fade,
                    progress: index as f32 / 49.0,
                }),
                [Some(3), None, None, None],
            )
            .map_err(|e| format!("Program composition: {e:?}"))?;
        let mut pcm = Vec::new();
        for _ in 0..recorder.required_audio_samples_for_frame(index)? {
            let tone = (sample_index as f64 * std::f64::consts::TAU * 1000.0
                / f64::from(DEFAULT_AUDIO_SAMPLE_RATE))
            .sin() as f32
                * 0.5;
            pcm.extend([tone, tone]);
            sample_index += 1;
        }
        recorder.write_frame_at(index, &composed, &pcm)?;
        // This example is offline and may be slower than real time.
        std::thread::sleep(Duration::from_millis(20));
    }
    let stats = recorder.finish(Duration::from_secs(30))?;
    println!("Saved {}: {} encoded frames, {} omitted timeline slots, {} audio sample frames; 1920x1080 50/1, stereo 48000 Hz. Wall time: {:?}. Synthetic CPU reference; real-time performance not established.",
        stats.path.display(), stats.encoded_video_frames, stats.missing_video_frames, stats.submitted_audio_samples_per_channel, started.elapsed());
    Ok(())
}
