//! Headless WebM recording of supplied Program RGBA and interleaved PCM.
//!
//! CPU VP8/Vorbis is an explicit proof-of-concept format, not a claim of
//! hardware encoding, real-time 1080p50, captured audio, or GPU composition.
//! Call from an output worker; finalization can wait for the encoder/muxer.

use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use openvideomix_core::{
    media_timebase::{audio_sample_pts_ns, video_frame_pts_ns},
    reference_compositor::RgbaFrame,
    video_profile::{VideoProfile, DEFAULT_AUDIO_SAMPLE_RATE},
};

const CHANNELS: u64 = 2;
const QUEUED_FRAMES: u64 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordingStats {
    pub path: PathBuf,
    /// Frames accepted from the caller, distinct from encoded buffers.
    pub submitted_video_frames: u64,
    pub encoded_video_frames: u64,
    pub submitted_audio_samples_per_channel: u64,
    /// Wall-clock time spent draining the encoder and muxer on stop.
    pub finalize_duration: Duration,
}

/// A single recording session. Any partial submission error terminates it;
/// the caller must start a fresh session rather than continue with A/V skew.
pub struct ProgramRecorder {
    pipeline: gst::Pipeline,
    video: gst_app::AppSrc,
    audio: gst_app::AppSrc,
    profile: VideoProfile,
    frame_bytes: u64,
    frames: u64,
    audio_samples: u64,
    encoded: Arc<AtomicU64>,
    path: PathBuf,
    failed: bool,
}

impl ProgramRecorder {
    /// Require VP8, Vorbis and WebM explicitly; never silently change codecs
    /// or the requested dimensions/rational frame rate. Existing files are
    /// refused. Missing plugins and invalid destinations are startup errors.
    pub fn start(path: &Path, profile: VideoProfile) -> Result<Self, String> {
        if profile.width == 0
            || profile.height == 0
            || profile.frame_rate.numerator == 0
            || profile.frame_rate.denominator == 0
            || profile.width > i32::MAX as u32
            || profile.height > i32::MAX as u32
            || profile.frame_rate.numerator > i32::MAX as u32
            || profile.frame_rate.denominator > i32::MAX as u32
        {
            return Err("Invalid recording video profile".into());
        }
        let frame_bytes = u64::from(profile.width)
            .checked_mul(u64::from(profile.height))
            .and_then(|pixels| pixels.checked_mul(4))
            .filter(|bytes| bytes.checked_mul(QUEUED_FRAMES).is_some())
            .ok_or("Recording frame size overflow")?;
        let location = path.to_str().ok_or("Recording path must be valid UTF-8")?;
        gst::init().map_err(|e| format!("GStreamer: {e}"))?;
        for name in [
            "appsrc",
            "queue",
            "videoconvert",
            "vp8enc",
            "audioconvert",
            "vorbisenc",
            "webmmux",
            "filesink",
        ] {
            if gst::ElementFactory::find(name).is_none() {
                return Err(format!("Required recording plugin is unavailable: {name}"));
            }
        }
        let pipeline = gst::parse::launch(
            "appsrc name=video format=time is-live=false block=false ! \
             queue max-size-buffers=4 max-size-bytes=0 max-size-time=0 ! \
             videoconvert ! vp8enc name=encoder deadline=1 cpu-used=8 ! mux. \
             appsrc name=audio format=time is-live=false block=false ! \
             queue max-size-buffers=4 max-size-bytes=0 max-size-time=0 ! \
             audioconvert ! vorbisenc ! mux. webmmux name=mux ! filesink name=output",
        )
        .map_err(|e| format!("Recording pipeline: {e}"))?
        .dynamic_cast::<gst::Pipeline>()
        .map_err(|_| "Invalid recording pipeline")?;
        let video = pipeline
            .by_name("video")
            .and_then(|e| e.dynamic_cast::<gst_app::AppSrc>().ok())
            .ok_or("Missing video appsrc")?;
        let audio = pipeline
            .by_name("audio")
            .and_then(|e| e.dynamic_cast::<gst_app::AppSrc>().ok())
            .ok_or("Missing audio appsrc")?;
        video.set_caps(Some(
            &gst::Caps::builder("video/x-raw")
                .field("format", "RGBA")
                .field("width", profile.width as i32)
                .field("height", profile.height as i32)
                .field("interlace-mode", "progressive")
                .field(
                    "framerate",
                    gst::Fraction::new(
                        profile.frame_rate.numerator as i32,
                        profile.frame_rate.denominator as i32,
                    ),
                )
                .build(),
        ));
        audio.set_caps(Some(
            &gst::Caps::builder("audio/x-raw")
                .field("format", "F32LE")
                .field("layout", "interleaved")
                .field("rate", DEFAULT_AUDIO_SAMPLE_RATE as i32)
                .field("channels", CHANNELS as i32)
                .build(),
        ));
        video.set_max_bytes(frame_bytes * QUEUED_FRAMES);
        audio.set_max_bytes(u64::from(DEFAULT_AUDIO_SAMPLE_RATE) * CHANNELS * 4);
        pipeline
            .by_name("output")
            .ok_or("Missing filesink")?
            .set_property("location", location);
        let encoded = Arc::new(AtomicU64::new(0));
        let counter = Arc::clone(&encoded);
        pipeline
            .by_name("encoder")
            .and_then(|e| e.static_pad("src"))
            .ok_or("Missing encoder output pad")?
            .add_probe(gst::PadProbeType::BUFFER, move |_, _| {
                counter.fetch_add(1, Ordering::Relaxed);
                gst::PadProbeReturn::Ok
            });
        // Reserve without truncation before allowing filesink to open it.
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| format!("Cannot create recording {}: {e}", path.display()))?;
        let recorder = Self {
            pipeline,
            video,
            audio,
            profile,
            frame_bytes,
            frames: 0,
            audio_samples: 0,
            encoded,
            path: path.to_owned(),
            failed: false,
        };
        recorder
            .pipeline
            .set_state(gst::State::Playing)
            .map_err(|e| format!("Cannot start recording: {e}"))?;
        Ok(recorder)
    }

    /// Exact number of PCM sample frames needed with the next video frame.
    /// Fractional rates alternate counts without accumulating rounded periods.
    pub fn next_audio_samples_per_channel(&self) -> Result<u64, String> {
        let next = self
            .frames
            .checked_add(1)
            .ok_or("Recording frame index overflow")?;
        let samples = u128::from(next)
            * u128::from(DEFAULT_AUDIO_SAMPLE_RATE)
            * u128::from(self.profile.frame_rate.denominator)
            / u128::from(self.profile.frame_rate.numerator);
        u64::try_from(samples)
            .ok()
            .and_then(|end| end.checked_sub(self.audio_samples))
            .ok_or_else(|| "Audio sample index overflow".into())
    }

    /// Supply the actual composited Program frame and actual PCM data. This
    /// method never captures a window or approximates the GUI display.
    /// Caller errors leave the session usable; downstream errors terminate it.
    pub fn write_frame(&mut self, frame: &RgbaFrame, pcm: &[f32]) -> Result<(), String> {
        if self.failed {
            return Err("Recording has failed".into());
        }
        let samples = self.next_audio_samples_per_channel()?;
        let expected_pcm = samples.checked_mul(CHANNELS).ok_or("PCM size overflow")?;
        if frame.width != self.profile.width
            || frame.height != self.profile.height
            || frame.pixels.len() as u64 != self.frame_bytes
            || pcm.len() as u64 != expected_pcm
            || pcm.iter().any(|sample| !sample.is_finite())
        {
            return Err("Program dimensions, packed RGBA, or 48 kHz stereo PCM do not match the recording profile".into());
        }
        let pts = video_frame_pts_ns(self.frames, self.profile.frame_rate)
            .ok_or("Video timestamp overflow")?;
        let next_pts = video_frame_pts_ns(
            self.frames.checked_add(1).ok_or("Frame index overflow")?,
            self.profile.frame_rate,
        )
        .ok_or("Video timestamp overflow")?;
        let end_samples = self
            .audio_samples
            .checked_add(samples)
            .ok_or("Audio timestamp overflow")?;
        let audio_pts = audio_sample_pts_ns(self.audio_samples, DEFAULT_AUDIO_SAMPLE_RATE)
            .ok_or("Audio timestamp overflow")?;
        let audio_end = audio_sample_pts_ns(end_samples, DEFAULT_AUDIO_SAMPLE_RATE)
            .ok_or("Audio timestamp overflow")?;
        let result = (|| {
            self.check_bus()?;
            if self
                .video
                .current_level_bytes()
                .checked_add(self.frame_bytes)
                .is_none_or(|bytes| bytes > self.video.max_bytes())
                || self
                    .audio
                    .current_level_bytes()
                    .checked_add(expected_pcm * 4)
                    .is_none_or(|bytes| bytes > self.audio.max_bytes())
            {
                return Err("Recording encoder backpressure: bounded queue is full".into());
            }
            let mut video = gst::Buffer::from_mut_slice(frame.pixels.clone());
            let buffer = video.get_mut().ok_or("Video buffer is not writable")?;
            buffer.set_pts(gst::ClockTime::from_nseconds(pts));
            buffer.set_duration(gst::ClockTime::from_nseconds(next_pts - pts));
            let bytes: Vec<u8> = pcm.iter().flat_map(|sample| sample.to_le_bytes()).collect();
            let mut audio = gst::Buffer::from_mut_slice(bytes);
            let buffer = audio.get_mut().ok_or("Audio buffer is not writable")?;
            buffer.set_pts(gst::ClockTime::from_nseconds(audio_pts));
            buffer.set_duration(gst::ClockTime::from_nseconds(audio_end - audio_pts));
            self.video
                .push_buffer(video)
                .map_err(|e| format!("Video encoder: {e}"))?;
            self.audio
                .push_buffer(audio)
                .map_err(|e| format!("Audio encoder: {e}"))?;
            Ok(())
        })();
        if let Err(error) = result {
            self.failed = true;
            let _ = self.pipeline.set_state(gst::State::Null);
            return Err(error);
        }
        self.frames += 1;
        self.audio_samples = end_samples;
        Ok(())
    }

    fn check_bus(&self) -> Result<(), String> {
        let bus = self.pipeline.bus().ok_or("Recording pipeline has no bus")?;
        if let Some(message) =
            bus.timed_pop_filtered(gst::ClockTime::ZERO, &[gst::MessageType::Error])
        {
            if let gst::MessageView::Error(error) = message.view() {
                return Err(format!("Recording failed: {}", error.error()));
            }
        }
        Ok(())
    }

    /// Send EOS on both streams and wait for the muxer to finalize the file.
    /// Timeout/error returns failure; it must never be reported as saved.
    pub fn finish(self, timeout: Duration) -> Result<RecordingStats, String> {
        if self.failed || self.frames == 0 {
            return Err("Recording failed or contains no frames".into());
        }
        let started = Instant::now();
        self.check_bus()?;
        self.video
            .end_of_stream()
            .map_err(|e| format!("Video EOS: {e}"))?;
        self.audio
            .end_of_stream()
            .map_err(|e| format!("Audio EOS: {e}"))?;
        let wait_ns =
            u64::try_from(timeout.as_nanos()).map_err(|_| "Invalid finalization timeout")?;
        let message = self
            .pipeline
            .bus()
            .ok_or("Missing recording bus")?
            .timed_pop_filtered(
                gst::ClockTime::from_nseconds(wait_ns),
                &[gst::MessageType::Eos, gst::MessageType::Error],
            )
            .ok_or("Recording finalization timed out")?;
        if let gst::MessageView::Error(error) = message.view() {
            return Err(format!("Recording finalization failed: {}", error.error()));
        }
        self.pipeline
            .set_state(gst::State::Null)
            .map_err(|e| format!("Recording shutdown: {e}"))?;
        Ok(RecordingStats {
            path: self.path.clone(),
            submitted_video_frames: self.frames,
            encoded_video_frames: self.encoded.load(Ordering::Relaxed),
            submitted_audio_samples_per_channel: self.audio_samples,
            finalize_duration: started.elapsed(),
        })
    }
}

impl Drop for ProgramRecorder {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoder_backpressure_terminates_session_instead_of_dropping_av() {
        let path = std::env::temp_dir().join(format!(
            "openvideomix-backpressure-{}.webm",
            std::process::id()
        ));
        let profile = VideoProfile {
            width: 64,
            height: 32,
            ..VideoProfile::default()
        };
        let mut recorder = ProgramRecorder::start(&path, profile).unwrap();
        recorder.video.set_max_bytes(1);
        let frame = RgbaFrame::solid(64, 32, [0, 0, 0, 255]).unwrap();
        let error = recorder.write_frame(&frame, &vec![0.0; 1920]).unwrap_err();
        assert!(error.contains("backpressure"));
        assert_eq!(recorder.frames, 0);
        assert_eq!(recorder.audio_samples, 0);
        assert!(recorder
            .write_frame(&frame, &vec![0.0; 1920])
            .unwrap_err()
            .contains("failed"));
        assert!(recorder.finish(Duration::from_secs(1)).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
