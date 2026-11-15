//! Restricted vMix shortcut-function compatibility adapter.
//!
//! This parser maps a verified subset of vMix shortcut names to the same
//! production commands as UI/MIDI/OpenAction. It does not expose vMix's
//! unauthenticated HTTP API, claim full protocol compatibility, or implement
//! visual transitions that the media engine cannot yet render.
//!
//! Reference: https://www.vmix.com/help29/DeveloperAPI.html

use crate::commands::ProductionCommand;
use crate::{InputId, MixerState};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ShortcutError {
    #[error("unsupported shortcut function: {0}")]
    UnsupportedFunction(String),
    #[error("a valid 1-based input number is required")]
    InvalidInput,
}

/// Input numbers in vMix are 1-based positions in its input rack, whereas
/// OPENVIDEOMIX uses stable numeric source IDs internally.
pub fn input_id_from_vmix_number(
    input_number: Option<&str>,
    state: &MixerState,
) -> Result<InputId, ShortcutError> {
    let index: usize = input_number
        .ok_or(ShortcutError::InvalidInput)?
        .parse()
        .map_err(|_| ShortcutError::InvalidInput)?;
    if index == 0 {
        return Err(ShortcutError::InvalidInput);
    }
    state
        .inputs
        .get(index - 1)
        .map(|source| source.id)
        .ok_or(ShortcutError::InvalidInput)
}

/// The subset is deliberately conservative. Unsupported operations fail
/// closed rather than silently degrading to a CUT or changing an overlay.
pub fn translate_shortcut(
    function: &str,
    input_number: Option<&str>,
    state: &MixerState,
) -> Result<ProductionCommand, ShortcutError> {
    match function {
        "Cut" if input_number.is_none() => Ok(ProductionCommand::Cut),
        "PreviewInput" => Ok(ProductionCommand::SelectPreview {
            input: input_id_from_vmix_number(input_number, state)?,
        }),
        "OverlayInput1In" => Ok(ProductionCommand::SetOverlay1 {
            input: Some(input_id_from_vmix_number(input_number, state)?),
        }),
        "OverlayInput1Off" if input_number.is_none() => {
            Ok(ProductionCommand::SetOverlay1 { input: None })
        }
        "AudioOn" => Ok(ProductionCommand::SetInputMute {
            input: input_id_from_vmix_number(input_number, state)?,
            muted: false,
        }),
        "AudioOff" => Ok(ProductionCommand::SetInputMute {
            input: input_id_from_vmix_number(input_number, state)?,
            muted: true,
        }),
        "Lock" if input_number.is_none() => Ok(ProductionCommand::SetSafetyLock { locked: true }),
        "Unlock" if input_number.is_none() => {
            Ok(ProductionCommand::SetSafetyLock { locked: false })
        }
        "ToggleSafetyLock" if input_number.is_none() => Ok(ProductionCommand::ToggleSafetyLock),
        _ => Err(ShortcutError::UnsupportedFunction(function.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::CommandDispatcher;

    #[test]
    fn vmix_input_numbers_follow_rack_order_not_internal_ids() {
        let mut state = MixerState::default();
        state.inputs[0].id = 900;
        let mut dispatcher = CommandDispatcher::default();
        let command = translate_shortcut("PreviewInput", Some("1"), &state).unwrap();
        assert_eq!(command, ProductionCommand::SelectPreview { input: 900 });
        dispatcher.dispatch(&mut state, command).unwrap();
        assert_eq!(state.preview_input, Some(900));
        assert_eq!(
            input_id_from_vmix_number(Some("0"), &state),
            Err(ShortcutError::InvalidInput)
        );
        assert_eq!(
            input_id_from_vmix_number(Some("-1"), &state),
            Err(ShortcutError::InvalidInput)
        );
        assert_eq!(
            input_id_from_vmix_number(Some("999"), &state),
            Err(ShortcutError::InvalidInput)
        );
    }

    #[test]
    fn repeated_overlay_in_is_not_a_toggle() {
        let mut state = MixerState::default();
        let mut dispatcher = CommandDispatcher::default();
        for _ in 0..2 {
            let command = translate_shortcut("OverlayInput1In", Some("5"), &state).unwrap();
            dispatcher.dispatch(&mut state, command).unwrap();
            assert_eq!(state.active_overlay_1, Some(5));
        }
        let command = translate_shortcut("OverlayInput1Off", None, &state).unwrap();
        dispatcher.dispatch(&mut state, command).unwrap();
        assert_eq!(state.active_overlay_1, None);
    }

    #[test]
    fn unsupported_transitions_are_never_silently_converted_to_cut() {
        let state = MixerState::default();
        assert!(matches!(
            translate_shortcut("Fade", None, &state),
            Err(ShortcutError::UnsupportedFunction(_))
        ));
        assert!(matches!(
            translate_shortcut("Cut", Some("3"), &state),
            Err(ShortcutError::UnsupportedFunction(_))
        ));
        assert_eq!(
            translate_shortcut("Cut", None, &state),
            Ok(ProductionCommand::Cut)
        );
    }

    #[test]
    fn audio_on_off_maps_to_explicit_mute_states() {
        let state = MixerState::default();
        assert_eq!(
            translate_shortcut("AudioOff", Some("2"), &state),
            Ok(ProductionCommand::SetInputMute {
                input: 2,
                muted: true
            })
        );
        assert_eq!(
            translate_shortcut("AudioOn", Some("2"), &state),
            Ok(ProductionCommand::SetInputMute {
                input: 2,
                muted: false
            })
        );
    }

    #[test]
    fn safety_lock_shortcuts_translate_correctly() {
        let state = MixerState::default();
        assert_eq!(
            translate_shortcut("Lock", None, &state),
            Ok(ProductionCommand::SetSafetyLock { locked: true })
        );
        assert_eq!(
            translate_shortcut("Unlock", None, &state),
            Ok(ProductionCommand::SetSafetyLock { locked: false })
        );
        assert_eq!(
            translate_shortcut("ToggleSafetyLock", None, &state),
            Ok(ProductionCommand::ToggleSafetyLock)
        );
    }
}
