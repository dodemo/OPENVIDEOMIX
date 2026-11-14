//! Platform-independent camera discovery and format negotiation.
//!
//! This module does not open hardware devices and must not be presented as a
//! functioning capture backend. OS adapters are implemented separately.
//! Camera identity, frame format negotiation and user-requested fixed framing
//! are deliberately vendor-agnostic.

use serde::{Deserialize, Serialize};

/// A rational frame rate. This avoids interpreting 59.94 as exactly 60 fps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameRate {
    pub numerator: u32,
    pub denominator: u32,
}

impl FrameRate {
    pub const PAL_50: Self = Self {
        numerator: 50,
        denominator: 1,
    };

    pub fn new(numerator: u32, denominator: u32) -> Option<Self> {
        if numerator == 0 || denominator == 0 {
            return None;
        }
        let d = gcd(numerator, denominator);
        Some(Self {
            numerator: numerator / d,
            denominator: denominator / d,
        })
    }

    pub fn equivalent(self, other: Self) -> bool {
        self.numerator as u64 * other.denominator as u64
            == other.numerator as u64 * self.denominator as u64
    }
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

/// Advertised formats must come from enumeration on the target machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PixelFormat {
    Mjpeg,
    H264,
    Yuy2,
    Nv12,
    Other(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraMode {
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    pub format: PixelFormat,
}

impl CameraMode {
    pub fn valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.frame_rate.numerator > 0
            && self.frame_rate.denominator > 0
    }
}

/// Device topology and serial are reported by platform adapters when present.
/// OS-specific ephemeral nodes such as /dev/video0 are not persistent IDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraIdentity {
    pub display_name: String,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub serial_number: Option<String>,
    pub topology_path: Option<String>,
    pub os_device_id: String,
}

impl CameraIdentity {
    /// A stable cross-reconnect key is only available when backed by a
    /// nonempty serial or a nonempty physical topology path.
    /// The OS device ID is intentionally not used as a persistent fallback.
    pub fn persistent_key(&self) -> Option<String> {
        if let (Some(vendor), Some(product), Some(serial)) = (
            self.vendor_id,
            self.product_id,
            self.serial_number
                .as_deref()
                .filter(|s| !s.trim().is_empty()),
        ) {
            return Some(format!("usb:{vendor:04x}:{product:04x}:serial:{serial}"));
        }
        let topology = self
            .topology_path
            .as_deref()
            .filter(|s| !s.trim().is_empty())?;
        Some(format!("port:{}", topology))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraCapabilities {
    pub device: CameraIdentity,
    pub modes: Vec<CameraMode>,
    pub pan_tilt: bool,
    pub hardware_tracking_control: bool,
}

/// Preferred format ordering is explicit; do not infer that MJPEG or H.264
/// is faster on every bus, GPU or OS.
pub fn select_mode(
    capabilities: &CameraCapabilities,
    width: u32,
    height: u32,
    frame_rate: FrameRate,
    preferred_formats: &[PixelFormat],
) -> Option<CameraMode> {
    capabilities
        .modes
        .iter()
        .filter(|mode| {
            mode.valid()
                && mode.width == width
                && mode.height == height
                && mode.frame_rate.equivalent(frame_rate)
        })
        .min_by_key(|mode| {
            preferred_formats
                .iter()
                .position(|format| *format == mode.format)
                .unwrap_or(usize::MAX)
        })
        .copied()
}

/// A request to keep a camera fixed; this is a desired state, not a claim
/// that the camera has accepted it or disabled vendor firmware gestures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixedCameraProfile {
    pub tracking_requested_off: bool,
    pub saved_position: Option<PanTiltZoom>,
}

impl Default for FixedCameraProfile {
    fn default() -> Self {
        Self {
            tracking_requested_off: true,
            saved_position: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PanTiltZoom {
    pub pan_degrees: f32,
    pub tilt_degrees: f32,
    /// Zoom is a dimensionless factor (e.g. 1.0 means no zoom).
    pub zoom: f32,
}

impl PanTiltZoom {
    /// Generic input validation only: vendor-specific mechanical limits and
    /// zoom steps must be obtained from the relevant device implementation.
    pub fn valid(&self) -> bool {
        self.pan_degrees.is_finite()
            && self.tilt_degrees.is_finite()
            && self.zoom.is_finite()
            && self.zoom >= 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn camera() -> CameraCapabilities {
        CameraCapabilities {
            device: CameraIdentity {
                display_name: "USB Camera".into(),
                vendor_id: Some(0x1234),
                product_id: Some(0xabcd),
                serial_number: Some("A001".into()),
                topology_path: Some("usb-1.3".into()),
                os_device_id: "/dev/video2".into(),
            },
            modes: vec![
                CameraMode {
                    width: 1920,
                    height: 1080,
                    frame_rate: FrameRate::new(50, 1).unwrap(),
                    format: PixelFormat::Mjpeg,
                },
                CameraMode {
                    width: 1920,
                    height: 1080,
                    frame_rate: FrameRate::new(100, 2).unwrap(),
                    format: PixelFormat::H264,
                },
                CameraMode {
                    width: 3840,
                    height: 2160,
                    frame_rate: FrameRate::new(30, 1).unwrap(),
                    format: PixelFormat::Mjpeg,
                },
            ],
            pan_tilt: false,
            hardware_tracking_control: false,
        }
    }

    #[test]
    fn rational_rates_are_normalized_and_validated() {
        assert_eq!(FrameRate::new(100, 2), Some(FrameRate::PAL_50));
        assert_eq!(FrameRate::new(0, 1), None);
        assert_eq!(FrameRate::new(50, 0), None);
        assert!(!FrameRate::new(60_000, 1001)
            .unwrap()
            .equivalent(FrameRate::new(60, 1).unwrap()));
    }

    #[test]
    fn selects_advertised_1080p50_in_preferred_encoding() {
        let selected = select_mode(
            &camera(),
            1920,
            1080,
            FrameRate::PAL_50,
            &[PixelFormat::H264, PixelFormat::Mjpeg],
        )
        .unwrap();
        assert_eq!(selected.format, PixelFormat::H264);
    }

    #[test]
    fn never_invents_unadvertised_4k50_mode() {
        assert!(select_mode(
            &camera(),
            3840,
            2160,
            FrameRate::PAL_50,
            &[PixelFormat::Mjpeg]
        )
        .is_none());
    }

    #[test]
    fn persistent_identity_ignores_ephemeral_device_path() {
        let mut device = camera().device;
        let original = device.persistent_key();
        device.os_device_id = "/dev/video9".into();
        assert_eq!(device.persistent_key(), original);
        device.vendor_id = None;
        assert_eq!(
            device.persistent_key(),
            Some("port:usb-1.3".into()),
            "Use physical topology when USB vendor/product IDs are unavailable"
        );
        device.vendor_id = Some(0x1234);
        device.serial_number = None;
        assert_eq!(device.persistent_key(), Some("port:usb-1.3".into()));
        device.topology_path = None;
        assert_eq!(device.persistent_key(), None);
    }

    #[test]
    fn fixed_mode_does_not_claim_hardware_control() {
        let fixed = FixedCameraProfile::default();
        assert!(fixed.tracking_requested_off);
        assert_eq!(fixed.saved_position, None);
        assert!(!camera().hardware_tracking_control);
        assert!(!PanTiltZoom {
            pan_degrees: f32::NAN,
            tilt_degrees: 0.0,
            zoom: 1.0
        }
        .valid());
    }
}
