//! Safe, backend-independent camera selection by persistent physical identity.
//!
//! An operating-system device path is a *connection handle*, not a stable
//! camera identity. Platform-specific discovery supplies the records; this
//! module only matches them and deliberately refuses ambiguous matches.

use crate::camera::{CameraCapabilities, CameraIdentity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraResolveError {
    /// The persisted key is missing or blank.
    InvalidKey,
    /// No currently connected device exposes the requested persistent key.
    NotConnected,
    /// More than one active device exposes the same persistent key.
    Ambiguous,
    /// A selected OS handle has no serial or physical topology identity.
    NotPersistent,
}

/// Resolve a saved identity to exactly one currently connected camera.
///
/// The adapter must re-enumerate devices after reconnect/sleep/wake. Never
/// silently substitute a different camera when the saved device disappears.
pub fn resolve_camera<'a>(
    devices: &'a [CameraCapabilities],
    saved_key: &str,
) -> Result<&'a CameraCapabilities, CameraResolveError> {
    if saved_key.is_empty() || saved_key.trim() != saved_key {
        return Err(CameraResolveError::InvalidKey);
    }

    let mut matches = devices
        .iter()
        .filter(|candidate| candidate.device.persistent_key().as_deref() == Some(saved_key));
    let first = matches.next().ok_or(CameraResolveError::NotConnected)?;
    if matches.next().is_some() {
        return Err(CameraResolveError::Ambiguous);
    }
    Ok(first)
}

/// Convert a *uniquely discovered* camera to a key suitable for saving.
///
/// Returns an error if the camera lacks physical identity or if a collision
/// exists. UI code must present these errors instead of saving a /dev/videoN
/// path or the first device with a matching display name.
pub fn persistent_selection(
    devices: &[CameraCapabilities],
    selected: &CameraIdentity,
) -> Result<String, CameraResolveError> {
    let key = selected
        .persistent_key()
        .ok_or(CameraResolveError::NotPersistent)?;
    let active = resolve_camera(devices, &key)?;
    if active.device.os_device_id != selected.os_device_id {
        return Err(CameraResolveError::NotConnected);
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::CameraCapabilities;

    fn camera(name: &str, serial: Option<&str>, node: &str) -> CameraCapabilities {
        CameraCapabilities {
            device: CameraIdentity {
                display_name: name.into(),
                vendor_id: Some(0x3564),
                product_id: Some(0xfef8),
                serial_number: serial.map(String::from),
                topology_path: Some("usb-3.2".into()),
                os_device_id: node.into(),
            },
            modes: vec![],
            pan_tilt: false,
            hardware_tracking_control: false,
        }
    }

    #[test]
    fn identity_survives_os_path_changes_and_duplicate_names() {
        let original = camera("USB Camera", Some("A"), "/dev/video0");
        let selected_key = persistent_selection(std::slice::from_ref(&original), &original.device).unwrap();
        let mut reconnected = original.clone();
        reconnected.device.os_device_id = "/dev/video8".into();
        let mut other = camera("USB Camera", Some("B"), "/dev/video0");
        other.device.topology_path = Some("usb-5.1".into());
        let devices = [other, reconnected];
        assert_eq!(
            resolve_camera(&devices, &selected_key)
                .unwrap()
                .device
                .os_device_id,
            "/dev/video8"
        );
    }

    #[test]
    fn ambiguous_hardware_keys_must_not_select_the_first_camera() {
        let one = camera("Cam 1", Some("same-serial"), "/dev/video0");
        let two = camera("Cam 2", Some("same-serial"), "/dev/video4");
        let devices = [one.clone(), two];
        assert!(matches!(
            persistent_selection(&devices, &one.device),
            Err(CameraResolveError::Ambiguous)
        ));
    }

    #[test]
    fn missing_camera_never_falls_back_to_another_device() {
        let saved = camera("Cam", Some("original"), "/dev/video0");
        let replacement = camera("Cam", Some("replacement"), "/dev/video0");
        let key = saved.device.persistent_key().unwrap();
        assert!(matches!(
            resolve_camera(&[replacement], &key),
            Err(CameraResolveError::NotConnected)
        ));
    }

    #[test]
    fn ephemeral_only_identity_cannot_be_saved() {
        let mut cam = camera("Cam", None, "/dev/video2");
        cam.device.topology_path = None;
        assert!(matches!(
            persistent_selection(&[cam.clone()], &cam.device),
            Err(CameraResolveError::NotPersistent)
        ));
    }

    #[test]
    fn cannot_save_a_stale_selection_from_a_previous_enumeration() {
        let active = camera("USB Camera", Some("A"), "/dev/video8");
        let stale = camera("USB Camera", Some("A"), "/dev/video0");
        assert!(matches!(
            persistent_selection(&[active], &stale.device),
            Err(CameraResolveError::NotConnected)
        ));
    }

    #[test]
    fn serialless_identical_usb_topologies_are_ambiguous() {
        let one = camera("Cam 1", None, "/dev/video0");
        let two = camera("Cam 2", None, "/dev/video1");
        let devices = [one.clone(), two];
        assert!(matches!(
            persistent_selection(&devices, &one.device),
            Err(CameraResolveError::Ambiguous)
        ));
    }

    #[test]
    fn refuse_empty_or_whitespace_modified_keys() {
        assert!(matches!(
            resolve_camera(&[], ""),
            Err(CameraResolveError::InvalidKey)
        ));
        assert!(matches!(
            resolve_camera(&[], " usb:3564:fef8:serial:A "),
            Err(CameraResolveError::InvalidKey)
        ));
    }
}
