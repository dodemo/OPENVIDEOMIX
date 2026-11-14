//! Default production video profile and explicit source capability checks.
//!
//! The output target is 1920x1080 progressive at 50/1 frames per second.
//! A source with a different native mode must be converted explicitly or
//! rejected; device enumeration is not proof that capture can sustain 50 fps.

use crate::camera::{CameraCapabilities, CameraMode, FrameRate, PixelFormat};

pub const DEFAULT_VIDEO_WIDTH: u32 = 1920;
pub const DEFAULT_VIDEO_HEIGHT: u32 = 1080;
pub const DEFAULT_VIDEO_FPS_NUMERATOR: u32 = 50;
pub const DEFAULT_VIDEO_FPS_DENOMINATOR: u32 = 1;
pub const DEFAULT_AUDIO_SAMPLE_RATE: u32 = 48_000;
pub const DEFAULT_OUTPUT_RESOLUTION_LABEL: &str = "1920x1080 (Full HD)";
pub const DEFAULT_OUTPUT_FRAME_RATE_LABEL: &str = "50.00 fps (PAL Broadcast 50Hz)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoProfile {
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
}

impl Default for VideoProfile {
    fn default() -> Self {
        Self {
            width: DEFAULT_VIDEO_WIDTH,
            height: DEFAULT_VIDEO_HEIGHT,
            frame_rate: FrameRate::PAL_50,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceModeDecision {
    /// Exact mode is advertised, not yet a measured/verified capture.
    Exact(CameraMode),
    /// No exact mode: conversion requires a separate user-visible decision.
    ConversionRequired,
    /// No usable modes were advertised by the backend.
    NoModes,
}

impl VideoProfile {
    /// Select an exact mode only. Never silently pick 30fps or lower
    /// resolution and report it as an unmodified 1080p50 feed.
    pub fn negotiate_exact(
        self,
        source: &CameraCapabilities,
        preferred_formats: &[PixelFormat],
    ) -> SourceModeDecision {
        if source.modes.iter().all(|mode| !mode.valid()) {
            return SourceModeDecision::NoModes;
        }
        match crate::camera::select_mode(
            source,
            self.width,
            self.height,
            self.frame_rate,
            preferred_formats,
        ) {
            Some(mode) => SourceModeDecision::Exact(mode),
            None => SourceModeDecision::ConversionRequired,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::CameraIdentity;

    fn capabilities(modes: Vec<CameraMode>) -> CameraCapabilities {
        CameraCapabilities {
            device: CameraIdentity {
                display_name: "Camera".into(),
                vendor_id: None,
                product_id: None,
                serial_number: None,
                topology_path: None,
                os_device_id: "mock-1".into(),
            },
            modes,
            pan_tilt: false,
            hardware_tracking_control: false,
        }
    }

    fn mode(width: u32, height: u32, fps: FrameRate) -> CameraMode {
        CameraMode {
            width,
            height,
            frame_rate: fps,
            format: PixelFormat::Mjpeg,
        }
    }

    #[test]
    fn default_is_full_hd_progressive_exactly_50_over_1() {
        let profile = VideoProfile::default();
        assert_eq!((profile.width, profile.height), (1920, 1080));
        assert_eq!(profile.frame_rate, FrameRate::PAL_50);
        assert_eq!(DEFAULT_AUDIO_SAMPLE_RATE, 48_000);
    }

    #[test]
    fn exact_1080p50_capability_is_selected() {
        let source = capabilities(vec![
            mode(1920, 1080, FrameRate::new(30, 1).unwrap()),
            mode(1920, 1080, FrameRate::PAL_50),
        ]);
        assert!(matches!(
            VideoProfile::default().negotiate_exact(&source, &[PixelFormat::Mjpeg]),
            SourceModeDecision::Exact(selected)
                if selected.width == 1920
                    && selected.height == 1080
                    && selected.frame_rate == FrameRate::PAL_50
        ));
    }

    #[test]
    fn thirty_fps_source_requires_explicit_conversion() {
        let source = capabilities(vec![mode(1920, 1080, FrameRate::new(30, 1).unwrap())]);
        assert_eq!(
            VideoProfile::default().negotiate_exact(&source, &[PixelFormat::Mjpeg]),
            SourceModeDecision::ConversionRequired
        );
    }

    #[test]
    fn reduced_resolution_is_not_misrepresented_as_full_hd() {
        let source = capabilities(vec![mode(640, 360, FrameRate::PAL_50)]);
        assert_eq!(
            VideoProfile::default().negotiate_exact(&source, &[]),
            SourceModeDecision::ConversionRequired
        );
    }

    #[test]
    fn fractional_5994_is_not_silently_accepted_as_50fps() {
        let source = capabilities(vec![mode(
            1920,
            1080,
            FrameRate::new(60_000, 1_001).unwrap(),
        )]);
        assert_eq!(
            VideoProfile::default().negotiate_exact(&source, &[PixelFormat::Mjpeg]),
            SourceModeDecision::ConversionRequired
        );
    }

    #[test]
    fn absent_capabilities_do_not_claim_support() {
        assert_eq!(
            VideoProfile::default().negotiate_exact(&capabilities(vec![]), &[]),
            SourceModeDecision::NoModes
        );
    }
}
