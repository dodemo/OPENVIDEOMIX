//! Bounded native 48 kHz stereo input on the existing video capture epoch.
//! No resampler, audio-master clock, device discovery, buses or AFV.
use crate::capture::{map_capture_pts_ns, CaptureTimeline};
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TrySendError},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct AudioSource {
    pub factory: String,
    pub device: Option<String>,
}
impl AudioSource {
    pub fn synthetic() -> Self {
        Self {
            factory: "audiotestsrc".into(),
            device: None,
        }
    }
    pub fn label(&self) -> String {
        if self.factory == "audiotestsrc" {
            "Synthetic live GStreamer audio".into()
        } else {
            format!(
                "Captured audio: {} / {}",
                self.factory,
                self.device.as_deref().unwrap_or("default device")
            )
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub enum AudioState {
    #[default]
    Starting,
    Capturing,
    Unavailable,
    Stopped,
    Error(String),
}
#[derive(Clone, Debug, Default)]
pub struct AudioStatus {
    pub state: AudioState,
    pub buffers: u64,
    pub queue_drops: u64,
    pub discontinuities: u64,
    pub peak_dbfs: Option<f32>,
    pub rms_dbfs: Option<f32>,
}
#[derive(Debug)]
pub struct AudioBlock {
    pub pts_ns: u64,
    pub pcm: Vec<f32>,
    pub discontinuity: bool,
}
pub struct AudioInput {
    pub receiver: Receiver<AudioBlock>,
    status: Arc<Mutex<AudioStatus>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl AudioInput {
    pub fn start(source: AudioSource) -> Result<Self, String> {
        gst::init().map_err(|e| e.to_string())?;
        let (sender, receiver) = mpsc::sync_channel(16);
        let status = Arc::new(Mutex::new(AudioStatus::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_status = Arc::clone(&status);
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("live-pcm-capture".into())
            .spawn(move || {
                let result = capture(source, &sender, &worker_stop, &worker_status);
                let mut status = worker_status.lock().unwrap_or_else(|p| p.into_inner());
                status.state = match result {
                    Ok(()) => AudioState::Stopped,
                    Err(e) => AudioState::Error(e),
                };
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            receiver,
            status,
            stop,
            worker: Some(worker),
        })
    }
    pub fn status(&self) -> AudioStatus {
        self.status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}
impl AudioInput {
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for AudioInput {
    fn drop(&mut self) {
        self.stop();
    }
}
struct Pipeline(gst::Pipeline);
impl Drop for Pipeline {
    fn drop(&mut self) {
        let _ = self.0.set_state(gst::State::Null);
    }
}
fn capture(
    source: AudioSource,
    sender: &mpsc::SyncSender<AudioBlock>,
    stop: &AtomicBool,
    status: &Mutex<AudioStatus>,
) -> Result<(), String> {
    // Only a source element is accepted: no arbitrary pipeline injection.
    let element = gst::ElementFactory::make(&source.factory)
        .build()
        .map_err(|e| e.to_string())?;
    if element.num_src_pads() != 1 || element.num_sink_pads() != 0 {
        return Err("Audio selection must be a single source element".into());
    }
    if let Some(device) = source.device {
        let property = element
            .find_property("device")
            .ok_or("This source has no device selector")?;
        let kind = property.value_type();
        if kind == String::static_type() {
            element.set_property("device", device);
        } else if kind == i32::static_type() {
            let value = device
                .parse::<i32>()
                .map_err(|_| "Audio device must be an integer")?;
            let spec = property
                .downcast_ref::<gst::glib::ParamSpecInt>()
                .ok_or("Invalid device property")?;
            if value < spec.minimum() || value > spec.maximum() {
                return Err("Audio device index out of range".into());
            }
            element.set_property("device", value);
        } else if kind == u32::static_type() {
            let value = device
                .parse::<u32>()
                .map_err(|_| "Audio device must be an unsigned integer")?;
            let spec = property
                .downcast_ref::<gst::glib::ParamSpecUInt>()
                .ok_or("Invalid device property")?;
            if value < spec.minimum() || value > spec.maximum() {
                return Err("Audio device index out of range".into());
            }
            element.set_property("device", value);
        } else {
            return Err("Unsupported device selector type".into());
        }
    }
    if source.factory == "audiotestsrc" {
        element.set_property("is-live", true);
        element.set_property("samplesperbuffer", 480_i32);
        element.set_property("volume", 0.5_f64);
    }
    capture_element(element, sender, stop, status)
}
fn capture_element(
    element: gst::Element,
    sender: &mpsc::SyncSender<AudioBlock>,
    stop: &AtomicBool,
    status: &Mutex<AudioStatus>,
) -> Result<(), String> {
    let pipeline = Pipeline(gst::Pipeline::new());
    // Rate/channel constraints precede conversion. Only PCM representation is
    // converted; unsupported native 48 kHz stereo fails negotiation explicitly.
    let native = gst::ElementFactory::make("capsfilter")
        .property(
            "caps",
            gst::Caps::builder("audio/x-raw")
                .field("rate", 48000_i32)
                .field("channels", 2_i32)
                .build(),
        )
        .build()
        .map_err(|e| e.to_string())?;
    let convert = gst::ElementFactory::make("audioconvert")
        .build()
        .map_err(|e| e.to_string())?;
    let sink = gst_app::AppSink::builder()
        .caps(
            &gst::Caps::builder("audio/x-raw")
                .field("format", "F32LE")
                .field("layout", "interleaved")
                .field("rate", 48000_i32)
                .field("channels", 2_i32)
                .build(),
        )
        .sync(false)
        .max_buffers(2)
        .drop(false)
        .build();
    pipeline
        .0
        .add_many([&element, &native, &convert, sink.upcast_ref()])
        .map_err(|e| e.to_string())?;
    gst::Element::link_many([&element, &native, &convert, sink.upcast_ref()])
        .map_err(|e| e.to_string())?;
    let timeline = CaptureTimeline::shared();
    pipeline.0.use_clock(Some(&timeline.clock));
    pipeline
        .0
        .set_state(gst::State::Playing)
        .map_err(|e| e.to_string())?;
    let bus = pipeline.0.bus().ok_or("Missing audio bus")?;
    let mut last = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        if let Some(message) = bus.timed_pop_filtered(
            gst::ClockTime::ZERO,
            &[gst::MessageType::Error, gst::MessageType::Eos],
        ) {
            return Err(match message.view() {
                gst::MessageView::Error(e) => format!("Audio capture: {}", e.error()),
                _ => "Audio source disconnected/EOS".into(),
            });
        }
        let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_mseconds(20)) else {
            if sink.is_eos() {
                return Err("Audio source disconnected/EOS".into());
            }
            if last.elapsed() > Duration::from_millis(500) {
                let mut s = status.lock().unwrap_or_else(|p| p.into_inner());
                s.state = AudioState::Unavailable;
                s.peak_dbfs = None;
                s.rms_dbfs = None;
            }
            continue;
        };
        let buffer = sample.buffer().ok_or("Audio buffer missing")?;
        let pts_ns = map_capture_pts_ns(
            buffer.pts(),
            sample.segment(),
            pipeline.0.base_time(),
            timeline.epoch_ns,
        )
        .ok_or("Audio shared timestamp unavailable")?;
        let map = buffer.map_readable().map_err(|e| e.to_string())?;
        if map.is_empty() || !map.len().is_multiple_of(8) || map.len() > 4800 * 8 {
            return Err("Audio PCM block exceeds 100 ms or has invalid stereo layout".into());
        }
        let pcm: Vec<f32> = map
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect();
        if pcm.iter().any(|v| !v.is_finite()) {
            return Err("Non-finite audio PCM".into());
        }
        let discontinuity = buffer.flags().contains(gst::BufferFlags::DISCONT);
        let (peak, rms) = levels(&pcm);
        let mut s = status.lock().unwrap_or_else(|p| p.into_inner());
        s.state = AudioState::Capturing;
        s.buffers += 1;
        s.discontinuities += u64::from(discontinuity);
        s.peak_dbfs = Some(peak);
        s.rms_dbfs = Some(rms);
        match sender.try_send(AudioBlock {
            pts_ns,
            pcm,
            discontinuity,
        }) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => s.queue_drops += 1,
            Err(TrySendError::Disconnected(_)) => return Ok(()),
        }
        last = Instant::now();
    }
    Ok(())
}
pub fn levels(pcm: &[f32]) -> (f32, f32) {
    if pcm.is_empty() {
        return (f32::NEG_INFINITY, f32::NEG_INFINITY);
    }
    let peak = pcm.iter().fold(0.0_f32, |a, v| a.max(v.abs()));
    let rms =
        (pcm.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / pcm.len() as f64).sqrt() as f32;
    (20.0 * peak.log10(), 20.0 * rms.log10())
}

#[derive(Clone, Debug, Default)]
pub struct AlignmentStats {
    pub silence_samples: u64,
    pub overlap_samples: u64,
    pub discontinuities: u64,
    pub max_timestamp_step_samples: u64,
    pub peak_dbfs: Option<f32>,
    pub rms_dbfs: Option<f32>,
}
/// Absolute PTS placement, nearest sample (<= 10.42 us quantization).
/// Missing data becomes counted silence; overlaps are counted/trimmed. No
/// resampling: >5 ms backward step or >1 s continuous underflow fails closed.
/// This is bounded timestamp repair, not physical device-clock discipline.
pub struct AudioTimeline {
    t0: u64,
    cursor: u64,
    pending: VecDeque<(u64, Vec<f32>)>,
    previous_end: Option<u64>,
    missing_run: u64,
    pub stats: AlignmentStats,
}
impl AudioTimeline {
    pub fn new(t0: u64) -> Self {
        Self {
            t0,
            cursor: 0,
            pending: VecDeque::new(),
            previous_end: None,
            missing_run: 0,
            stats: AlignmentStats::default(),
        }
    }
    pub fn ingest(&mut self, block: AudioBlock) -> Result<(), String> {
        if self.pending.len() >= 128 {
            return Err("Audio alignment queue full".into());
        }
        if block.pcm.is_empty()
            || !block.pcm.len().is_multiple_of(2)
            || block.pcm.len() > 9600
            || block.pcm.iter().any(|v| !v.is_finite())
        {
            return Err("Invalid audio alignment block".into());
        }
        let frames = block.pcm.len() as u64 / 2;
        // Signed arithmetic preserves sources that started before recording T0.
        let offset = i128::from(block.pts_ns) - i128::from(self.t0);
        let start = (offset * 48000
            + if offset >= 0 {
                500_000_000
            } else {
                -500_000_000
            })
            / 1_000_000_000;
        let end = start
            .checked_add(i128::from(frames))
            .ok_or("Audio time overflow")?;
        if end <= 0 {
            return Ok(());
        }
        let mut trim = if start < 0 {
            usize::try_from(-start).map_err(|_| "Audio trim overflow")?
        } else {
            0
        };
        let mut start = u64::try_from(start.max(0)).map_err(|_| "Audio time overflow")?;
        let end = u64::try_from(end).map_err(|_| "Audio time overflow")?;
        if let Some(previous) = self.previous_end {
            let step = start.abs_diff(previous);
            self.stats.max_timestamp_step_samples = self.stats.max_timestamp_step_samples.max(step);
            if previous > start && step > 240 {
                return Err("Audio backward timestamp/drift exceeds 5 ms correction bound".into());
            }
        }
        let accepted_start = start
            .max(self.previous_end.unwrap_or(0))
            .max(self.cursor)
            .min(end);
        let overlap = accepted_start - start;
        self.stats.overlap_samples += overlap;
        trim += overlap as usize;
        start = accepted_start;
        self.previous_end = Some(self.previous_end.unwrap_or(0).max(end));
        self.stats.discontinuities += u64::from(block.discontinuity);
        if start < end {
            self.pending
                .push_back((start, block.pcm[trim * 2..].to_vec()));
        }
        Ok(())
    }
    pub fn render(&mut self, count: u64, gain: f32) -> Result<Vec<f32>, String> {
        if count > 48000 || !gain.is_finite() || !(0.0..=1.0).contains(&gain) {
            return Err("Invalid bounded audio transaction/gain".into());
        }
        let end = self
            .cursor
            .checked_add(count)
            .ok_or("Audio cursor overflow")?;
        let mut pcm = vec![0.0; count as usize * 2];
        let mut covered = vec![false; count as usize];
        while let Some((start, data)) = self.pending.front() {
            let block_end = start
                .checked_add(data.len() as u64 / 2)
                .ok_or("Audio block overflow")?;
            if *start >= end {
                break;
            }
            let from = self.cursor.max(*start);
            let to = end.min(block_end);
            if to > from {
                let source = ((from - start) * 2) as usize;
                let destination = ((from - self.cursor) * 2) as usize;
                let length = ((to - from) * 2) as usize;
                for (out, value) in pcm[destination..destination + length]
                    .iter_mut()
                    .zip(&data[source..source + length])
                {
                    *out = *value * gain;
                }
                covered[(from - self.cursor) as usize..(to - self.cursor) as usize].fill(true);
            }
            if block_end <= end {
                self.stats.overlap_samples += self
                    .cursor
                    .saturating_sub(*start)
                    .min(data.len() as u64 / 2);
                self.pending.pop_front();
            } else {
                // Retain only the unconsumed tail, so previous consumption is
                // not counted as an overlap when the next transaction starts.
                let consumed = ((end - start) * 2) as usize;
                let tail = data[consumed..].to_vec();
                self.pending.pop_front();
                self.pending.push_front((end, tail));
                break;
            }
        }
        for present in covered {
            if present {
                self.missing_run = 0;
            } else {
                self.stats.silence_samples += 1;
                self.missing_run += 1;
                if self.missing_run > 48000 {
                    return Err("Audio unavailable for more than one second".into());
                }
            }
        }
        let (peak, rms) = levels(&pcm);
        self.stats.peak_dbfs = Some(peak);
        self.stats.rms_dbfs = Some(rms);
        self.cursor = end;
        Ok(pcm)
    }
    /// Wait only on the encoder consumer, never on capture/GUI/video. The
    /// lookahead is capped at 128 blocks; absent audio waits at most 100 ms.
    pub fn collect(
        &mut self,
        input: &AudioInput,
        count: u64,
        gain: f32,
    ) -> Result<Vec<f32>, String> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or("Audio cursor overflow")?;
        let deadline = Instant::now() + Duration::from_millis(100);
        while self
            .pending
            .back()
            .is_none_or(|(s, d)| s.saturating_add(d.len() as u64 / 2) < end)
        {
            if let AudioState::Error(e) = input.status().state {
                return Err(e);
            }
            if self.pending.len() >= 128 {
                return Err("Audio alignment lookahead exceeds bounded queue".into());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match input.receiver.recv_timeout(remaining) {
                Ok(block) => self.ingest(block)?,
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Audio capture disconnected".into())
                }
            }
        }
        self.render(count, gain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_eos_is_an_error_and_pipeline_cleanup_does_not_need_a_consumer() {
        gst::init().unwrap();
        let element = gst::ElementFactory::make("audiotestsrc")
            .property("is-live", true)
            .property("num-buffers", 2_i32)
            .property("samplesperbuffer", 480_i32)
            .build()
            .unwrap();
        let (tx, rx) = mpsc::sync_channel(1);
        let status = Mutex::new(AudioStatus::default());
        let error = capture_element(element, &tx, &AtomicBool::new(false), &status).unwrap_err();
        assert!(error.contains("EOS"), "{error}");
        assert_eq!(status.lock().unwrap().queue_drops, 1);
        assert_eq!(rx.try_iter().count(), 1);
    }
    #[test]
    fn native_wrong_rate_is_rejected_without_resampling() {
        gst::init().unwrap();
        let source = gst::parse::bin_from_description(
            r#"audiotestsrc is-live=true ! capsfilter caps="audio/x-raw,rate=44100,channels=2""#,
            true,
        )
        .unwrap();
        let (tx, _rx) = mpsc::sync_channel(1);
        let status = Mutex::new(AudioStatus::default());
        assert!(capture_element(source.upcast(), &tx, &AtomicBool::new(false), &status).is_err());
    }
}
