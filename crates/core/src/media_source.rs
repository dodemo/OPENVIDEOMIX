//! Real media sources: typed source lifecycle, ingest health and playback state.
//!
//! Replaces mock placeholders with a deterministic state machine for cameras,
//! screen portals and file clips across Windows, Linux and macOS.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::camera::{
    CameraCapabilities, CameraIdentity, CameraMode, FixedCameraProfile, FrameRate, PanTiltZoom,
    PixelFormat,
};

/// Known USB Vendor & Product IDs for OBSBOT Tiny 2 webcam.
pub const OBSBOT_TINY2_VID: u16 = 0x3564;
pub const OBSBOT_TINY2_PID: u16 = 0xfef8;

/// Mechanical and firmware limits for OBSBOT Tiny 2 gimbal.
pub const OBSBOT_TINY2_PAN_MIN: f32 = -140.0;
pub const OBSBOT_TINY2_PAN_MAX: f32 = 140.0;
pub const OBSBOT_TINY2_TILT_MIN: f32 = -30.0;
pub const OBSBOT_TINY2_TILT_MAX: f32 = 70.0;
pub const OBSBOT_TINY2_ZOOM_MIN: f32 = 1.0;
pub const OBSBOT_TINY2_ZOOM_MAX: f32 = 4.0;

/// Specific error conditions for physical and file media sources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
pub enum SourceError {
    #[error("device '{0}' not found or disconnected")]
    DeviceNotFound(String),
    #[error("format unsupported: requested {requested}, available: {available:?}")]
    FormatUnsupported {
        requested: String,
        available: Vec<String>,
    },
    #[error("OS permission denied: {0}")]
    PermissionDenied(String),
    #[error("device was hot-unplugged: {0}")]
    HotUnplugged(String),
    #[error("media decoder error: {0}")]
    DecoderError(String),
    #[error("timeout waiting for frames after {elapsed_ms}ms")]
    Timeout { elapsed_ms: u64 },
}

/// Typed media source lifecycle states.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SourceLifecycle {
    /// Device or file discovered during OS enumeration.
    Discovered,
    /// Format, framerate and profile negotiated.
    Configured { mode: CameraMode },
    /// Opening pipeline / device handle with timestamp.
    Opening {
        mode: CameraMode,
        started_at: DateTime<Utc>,
    },
    /// Live frame delivery active.
    Live {
        mode: CameraMode,
        started_at: DateTime<Utc>,
    },
    /// Frame delivery ceased or stalled beyond acceptable threshold.
    Stalled {
        last_frame_at: DateTime<Utc>,
        timeout_ms: u64,
    },
    /// Fatal or recoverable failure.
    Error { reason: SourceError },
    /// Cleanly released and closed.
    Closed,
}

impl SourceLifecycle {
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Live { .. })
    }

    pub fn is_operational(&self) -> bool {
        matches!(
            self,
            Self::Configured { .. } | Self::Opening { .. } | Self::Live { .. }
        )
    }

    pub fn can_reconnect(&self) -> bool {
        matches!(
            self,
            Self::Stalled { .. } | Self::Error { .. } | Self::Closed
        )
    }
}

/// Operational health and queue accounting for a media ingest stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngestHealth {
    pub frames_received: u64,
    pub frames_dropped_queue_full: u64,
    pub frames_corrupted: u64,
    pub last_pts_ns: Option<u64>,
    pub measured_fps: f32,
    pub status: IngestStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IngestStatus {
    Healthy,
    Degraded { reason: String },
    Failing { reason: String },
}

impl Default for IngestHealth {
    fn default() -> Self {
        Self {
            frames_received: 0,
            frames_dropped_queue_full: 0,
            frames_corrupted: 0,
            last_pts_ns: None,
            measured_fps: 0.0,
            status: IngestStatus::Healthy,
        }
    }
}

impl IngestHealth {
    pub fn record_frame(&mut self, pts_ns: Option<u64>) {
        self.frames_received = self.frames_received.saturating_add(1);
        self.last_pts_ns = pts_ns;
        self.update_status();
    }

    pub fn record_drop(&mut self) {
        self.frames_dropped_queue_full = self.frames_dropped_queue_full.saturating_add(1);
        self.update_status();
    }

    pub fn record_corruption(&mut self) {
        self.frames_corrupted = self.frames_corrupted.saturating_add(1);
        self.update_status();
    }

    fn update_status(&mut self) {
        if self.frames_corrupted > 10 {
            self.status = IngestStatus::Failing {
                reason: format!("High corruption rate: {} bad frames", self.frames_corrupted),
            };
        } else if self.frames_dropped_queue_full > 5 {
            self.status = IngestStatus::Degraded {
                reason: format!(
                    "Queue backlog: {} dropped frames",
                    self.frames_dropped_queue_full
                ),
            };
        } else {
            self.status = IngestStatus::Healthy;
        }
    }
}

/// Deterministic playback controller for pre-recorded media files (MP4, MKV, WebM).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaPlaybackState {
    pub path: String,
    pub duration_ms: u64,
    pub position_ms: u64,
    pub is_playing: bool,
    pub is_looping: bool,
    pub has_audio: bool,
    pub video_width: u32,
    pub video_height: u32,
}

impl MediaPlaybackState {
    pub fn new(
        path: impl Into<String>,
        duration_ms: u64,
        video_width: u32,
        video_height: u32,
        has_audio: bool,
    ) -> Self {
        Self {
            path: path.into(),
            duration_ms,
            position_ms: 0,
            is_playing: false,
            is_looping: true,
            has_audio,
            video_width,
            video_height,
        }
    }

    pub fn play(&mut self) {
        self.is_playing = true;
    }

    pub fn pause(&mut self) {
        self.is_playing = false;
    }

    pub fn seek(&mut self, position_ms: u64) {
        self.position_ms = position_ms.min(self.duration_ms);
    }

    pub fn toggle_loop(&mut self) {
        self.is_looping = !self.is_looping;
    }

    /// Advance playback clock by elapsed_ms. Returns true if stream looped or ended.
    pub fn advance(&mut self, elapsed_ms: u64) -> bool {
        if !self.is_playing || self.duration_ms == 0 {
            return false;
        }

        let new_pos = self.position_ms.saturating_add(elapsed_ms);
        if new_pos >= self.duration_ms {
            if self.is_looping {
                self.position_ms = new_pos % self.duration_ms;
                true
            } else {
                self.position_ms = self.duration_ms;
                self.is_playing = false;
                true
            }
        } else {
            self.position_ms = new_pos;
            false
        }
    }
}

/// Helper for OBSBOT Tiny 2 camera capabilities and fixed framing validation.
pub struct ObsbotTiny2Profile;

impl ObsbotTiny2Profile {
    /// Check whether a discovered camera is an OBSBOT Tiny 2 device.
    pub fn is_matching_device(identity: &CameraIdentity) -> bool {
        if let (Some(vid), Some(pid)) = (identity.vendor_id, identity.product_id) {
            if vid == OBSBOT_TINY2_VID && pid == OBSBOT_TINY2_PID {
                return true;
            }
        }
        identity.display_name.to_lowercase().contains("tiny 2")
            || identity.display_name.to_lowercase().contains("obsbot")
    }

    /// Validate Pan/Tilt/Zoom coordinates within OBSBOT Tiny 2 mechanical boundaries.
    pub fn validate_ptz(pan: f32, tilt: f32, zoom: f32) -> Result<PanTiltZoom, &'static str> {
        if !pan.is_finite() || !(OBSBOT_TINY2_PAN_MIN..=OBSBOT_TINY2_PAN_MAX).contains(&pan) {
            return Err("Pan angle out of mechanical limits (-140° to +140°)");
        }
        if !tilt.is_finite() || !(OBSBOT_TINY2_TILT_MIN..=OBSBOT_TINY2_TILT_MAX).contains(&tilt) {
            return Err("Tilt angle out of mechanical limits (-30° to +70°)");
        }
        if !zoom.is_finite() || !(OBSBOT_TINY2_ZOOM_MIN..=OBSBOT_TINY2_ZOOM_MAX).contains(&zoom) {
            return Err("Zoom factor out of range (1.0x to 4.0x)");
        }
        Ok(PanTiltZoom {
            pan_degrees: pan,
            tilt_degrees: tilt,
            zoom,
        })
    }

    /// Build a safe fixed-camera profile for broadcast (AI tracking off, no automatic movement).
    pub fn default_fixed_profile() -> FixedCameraProfile {
        FixedCameraProfile {
            tracking_requested_off: true,
            saved_position: Some(PanTiltZoom {
                pan_degrees: 0.0,
                tilt_degrees: 0.0,
                zoom: 1.0,
            }),
        }
    }

    /// Select standard 1080p50 mode for OBSBOT Tiny 2 if advertised.
    pub fn select_broadcast_mode(capabilities: &CameraCapabilities) -> Option<CameraMode> {
        crate::camera::select_mode(
            capabilities,
            1920,
            1080,
            FrameRate::PAL_50,
            &[
                PixelFormat::Mjpeg,
                PixelFormat::H264,
                PixelFormat::Nv12,
                PixelFormat::Yuy2,
            ],
        )
    }
}

/// Bounded queue for real-time video frames that drops oldest on overflow.
#[derive(Debug)]
pub struct BoundedFrameQueue<T> {
    capacity: usize,
    buffer: std::collections::VecDeque<T>,
    total_enqueued: u64,
    total_dropped: u64,
}

impl<T> BoundedFrameQueue<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be non-zero");
        Self {
            capacity,
            buffer: std::collections::VecDeque::with_capacity(capacity),
            total_enqueued: 0,
            total_dropped: 0,
        }
    }

    /// Push a frame into the queue. If capacity is exceeded, drops the oldest frame.
    /// Returns true if a frame was dropped.
    pub fn push(&mut self, item: T) -> bool {
        self.total_enqueued = self.total_enqueued.saturating_add(1);
        let dropped = if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
            self.total_dropped = self.total_dropped.saturating_add(1);
            true
        } else {
            false
        };
        self.buffer.push_back(item);
        dropped
    }

    pub fn pop(&mut self) -> Option<T> {
        self.buffer.pop_front()
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn total_enqueued(&self) -> u64 {
        self.total_enqueued
    }

    pub fn total_dropped(&self) -> u64 {
        self.total_dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_lifecycle_transitions() {
        let mode = CameraMode {
            width: 1920,
            height: 1080,
            frame_rate: FrameRate::PAL_50,
            format: PixelFormat::Mjpeg,
        };

        let state = SourceLifecycle::Discovered;
        assert!(!state.is_live());
        assert!(!state.is_operational());

        let state = SourceLifecycle::Configured { mode };
        assert!(!state.is_live());
        assert!(state.is_operational());

        let live_state = SourceLifecycle::Live {
            mode,
            started_at: Utc::now(),
        };
        assert!(live_state.is_live());
        assert!(live_state.is_operational());
        assert!(!live_state.can_reconnect());

        let err_state = SourceLifecycle::Error {
            reason: SourceError::HotUnplugged("USB bus reset".into()),
        };
        assert!(!err_state.is_live());
        assert!(!err_state.is_operational());
        assert!(err_state.can_reconnect());
    }

    #[test]
    fn ingest_health_accounting() {
        let mut health = IngestHealth::default();
        assert_eq!(health.status, IngestStatus::Healthy);

        for _ in 0..100 {
            health.record_frame(Some(123456789));
        }
        assert_eq!(health.frames_received, 100);
        assert_eq!(health.status, IngestStatus::Healthy);

        for _ in 0..6 {
            health.record_drop();
        }
        assert!(matches!(health.status, IngestStatus::Degraded { .. }));

        for _ in 0..15 {
            health.record_corruption();
        }
        assert!(matches!(health.status, IngestStatus::Failing { .. }));
    }

    #[test]
    fn media_playback_state_looping_and_seek() {
        let mut clip = MediaPlaybackState::new("assets/opener.mp4", 10_000, 1920, 1080, true);
        assert!(!clip.is_playing);
        assert_eq!(clip.position_ms, 0);

        clip.play();
        assert!(clip.is_playing);

        // Advance 5 seconds
        let finished = clip.advance(5_000);
        assert!(!finished);
        assert_eq!(clip.position_ms, 5_000);

        // Advance another 6 seconds (exceeding 10s duration) -> should loop back to 1s
        let looped = clip.advance(6_000);
        assert!(looped);
        assert_eq!(clip.position_ms, 1_000);

        // Disable loop and advance past end
        clip.toggle_loop();
        assert!(!clip.is_looping);
        clip.advance(15_000);
        assert_eq!(clip.position_ms, 10_000);
        assert!(!clip.is_playing);

        // Seek
        clip.seek(3_500);
        assert_eq!(clip.position_ms, 3_500);
    }

    #[test]
    fn obsbot_tiny2_identity_and_ptz_validation() {
        let identity = CameraIdentity {
            display_name: "OBSBOT Tiny 2 Camera".into(),
            vendor_id: Some(OBSBOT_TINY2_VID),
            product_id: Some(OBSBOT_TINY2_PID),
            serial_number: Some("TINY2-98765".into()),
            topology_path: Some("1-2.4".into()),
            os_device_id: "/dev/video4".into(),
        };

        assert!(ObsbotTiny2Profile::is_matching_device(&identity));

        // Valid PTZ coordinates
        let ptz = ObsbotTiny2Profile::validate_ptz(45.0, 15.0, 2.0).unwrap();
        assert_eq!(ptz.pan_degrees, 45.0);
        assert_eq!(ptz.tilt_degrees, 15.0);
        assert_eq!(ptz.zoom, 2.0);

        // Out-of-bounds pan
        assert!(ObsbotTiny2Profile::validate_ptz(180.0, 0.0, 1.0).is_err());
        // Out-of-bounds tilt
        assert!(ObsbotTiny2Profile::validate_ptz(0.0, -45.0, 1.0).is_err());
        // Out-of-bounds zoom
        assert!(ObsbotTiny2Profile::validate_ptz(0.0, 0.0, 5.0).is_err());

        let fixed = ObsbotTiny2Profile::default_fixed_profile();
        assert!(fixed.tracking_requested_off);
        assert_eq!(fixed.saved_position.unwrap().pan_degrees, 0.0);
    }

    #[test]
    fn bounded_frame_queue_drops_oldest() {
        let mut queue = BoundedFrameQueue::<u32>::new(3);
        assert_eq!(queue.len(), 0);

        assert!(!queue.push(10));
        assert!(!queue.push(20));
        assert!(!queue.push(30));
        assert_eq!(queue.len(), 3);
        assert_eq!(queue.total_dropped(), 0);

        // 4th item drops oldest (10)
        assert!(queue.push(40));
        assert_eq!(queue.len(), 3);
        assert_eq!(queue.total_dropped(), 1);
        assert_eq!(queue.pop(), Some(20));
        assert_eq!(queue.pop(), Some(30));
        assert_eq!(queue.pop(), Some(40));
        assert_eq!(queue.pop(), None);
    }
}
