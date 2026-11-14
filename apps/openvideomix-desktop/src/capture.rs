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

use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use gst_video::prelude::*;

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
            if let Err(message) = run_capture(&sender, &worker_stop, "autovideosrc") {
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

fn run_capture(sender: &SyncSender<CameraEvent>, stop: &AtomicBool, source: &str) -> Result<(), String> {
    gst::init().map_err(|e| format!("GStreamer: {e}"))?;

    // A conservative working format is used until device capability
    // enumeration and hardware-dependent negotiation are implemented.
    // 'autovideosrc' chooses a platform-specific camera source.
    let pipeline_description = format!(
        "{source} ! videoconvert ! videoscale ! videorate ! \
         video/x-raw,format=RGBA,width=640,height=360,framerate=30/1 ! \
         appsink name=framesink emit-signals=false sync=false max-buffers=2 drop=true"
    );
    let element = gst::parse::launch(&pipeline_description)
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
        let Ok(video_frame) = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info) else {
            continue;
        };
        let Ok(plane) = video_frame.plane_data(0) else {
            continue;
        };
        let stride = video_frame.plane_stride()[0];
        let Some(rgba) = copy_packed_rgba(plane, width, height, stride) else {
            continue;
        };

        match sender.try_send(CameraEvent::Frame(CameraFrame {
            width,
            height,
            rgba,
        })) {
            Ok(()) | Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => break Ok(()),
        }
    };

    let _ = pipeline.set_state(gst::State::Null);
    result
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
    fn packed_rgba_handles_row_padding_and_rejects_bad_strides() {
        let padded = [1, 2, 3, 4, 5, 6, 7, 8, 99, 99, 9, 10, 11, 12, 13, 14, 15, 16];
        assert_eq!(
            copy_packed_rgba(&padded, 2, 2, 10),
            None,
            "A última fila incompleta non se debe interpretar como válida"
        );
        let padded = [1, 2, 3, 4, 5, 6, 7, 8, 99, 99, 9, 10, 11, 12, 13, 14, 15, 16];
        assert_eq!(
            copy_packed_rgba(&padded, 2, 2, 10),
            None
        );
        let padded = [1, 2, 3, 4, 5, 6, 7, 8, 99, 99, 9, 10, 11, 12, 13, 14, 15, 16, 0, 0];
        assert_eq!(
            copy_packed_rgba(&padded, 2, 2, 10),
            Some(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16])
        );
        assert!(copy_packed_rgba(&padded, 2, 2, -1).is_none());
        assert!(copy_packed_rgba(&padded, 2, 2, 7).is_none());
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
