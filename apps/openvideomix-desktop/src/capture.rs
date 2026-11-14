//! Minimal real USB-camera ingestion via GStreamer.
//!
//! This opt-in proof of concept uses autovideosrc, so it selects a system
//! default camera rather than a specific persistent device ID. It is not yet
//! a production 1080p50 or multi-camera capture implementation.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender, TrySendError},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;

pub struct CameraFrame {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
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
        let (sender, receiver) = mpsc::sync_channel(3);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            if let Err(message) = run_capture(&sender, &worker_stop) {
                let _ = sender.try_send(CameraEvent::Error(message));
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

fn run_capture(sender: &SyncSender<CameraEvent>, stop: &AtomicBool) -> Result<(), String> {
    gst::init().map_err(|e| format!("GStreamer: {e}"))?;

    // A conservative working format is used until device capability
    // enumeration and hardware-dependent negotiation are implemented.
    // 'autovideosrc' chooses a platform-specific camera source.
    let element = gst::parse::launch(
        "autovideosrc ! videoconvert ! videoscale ! videorate ! \
         video/x-raw,format=RGBA,width=640,height=360,framerate=30/1 ! \
         appsink name=framesink emit-signals=false sync=false max-buffers=2 drop=true",
    )
    .map_err(|e| format!("Non se puido crear o pipeline de webcam: {e}"))?;

    let pipeline = element
        .dynamic_cast::<gst::Pipeline>()
        .map_err(|_| "O pipeline de captura non e unha Pipeline".to_string())?;
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

    let result = loop {
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
            continue;
        };
        let Some(caps) = sample.caps() else {
            continue;
        };
        let Some(structure) = caps.structure(0) else {
            continue;
        };
        let Ok(width) = structure.get::<i32>("width") else {
            continue;
        };
        let Ok(height) = structure.get::<i32>("height") else {
            continue;
        };
        if width <= 0 || height <= 0 {
            continue;
        }
        let Some(buffer) = sample.buffer() else {
            continue;
        };
        let Ok(map) = buffer.map_readable() else {
            continue;
        };
        let Some(bytes) = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
        else {
            continue;
        };
        // This PoC negotiates RGBA; reject any unexpectedly sized buffers
        // instead of showing corrupted imagery. Row strides require a
        // video-frame mapping before supporting arbitrary source layouts.
        if map.as_slice().len() != bytes {
            continue;
        }

        match sender.try_send(CameraEvent::Frame(CameraFrame {
            width: width as usize,
            height: height as usize,
            rgba: map.as_slice().to_vec(),
        })) {
            Ok(()) | Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => break Ok(()),
        }
    };

    let _ = pipeline.set_state(gst::State::Null);
    result
}
