//! Device-neutral production commands. The GUI, keyboard, MIDI and OpenDeck
//! must dispatch these actions through the same validated API.
//!
//! Network transport and authorization are deliberately out of scope here:
//! callers must authenticate and authorize **before** invoking dispatch.

use crate::{InputId, MixerState};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable public contract for control surfaces. Add new variants without
/// reusing existing meanings, and version network protocols separately.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", content = "args", rename_all = "snake_case")]
pub enum ProductionCommand {
    SelectPreview {
        input: InputId,
    },
    SelectProgram {
        input: InputId,
    },
    Cut,
    StartTestRecording {
        path: String,
    },
    StartLiveRecording {
        path: String,
        factory: String,
        device: Option<String>,
    },
    StopRecording,
    ToggleOverlay {
        channel: u8,
        input: InputId,
    },
    SetOverlay {
        channel: u8,
        input: Option<InputId>,
    },
    ToggleOverlay1 {
        input: InputId,
    },
    SetOverlay1 {
        input: Option<InputId>,
    },
    SetInputVolume {
        input: InputId,
        volume: f32,
    },
    SetInputMute {
        input: InputId,
        muted: bool,
    },
    SetInputAfv {
        input: InputId,
        enabled: bool,
    },
    SetMasterVolume {
        volume: f32,
    },
    SetMasterMute {
        muted: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CommandError {
    #[error("input {0} does not exist")]
    UnknownInput(InputId),
    #[error("overlay channel {0} must be between 1 and 4")]
    InvalidOverlayChannel(u8),
    #[error("volume must be a finite number between 0 and 1")]
    InvalidVolume,
    #[error("there is no preview source to take to Program")]
    NoPreview,
    #[error("recording path must be nonempty and contain no NUL")]
    InvalidRecordingPath,
    #[error("invalid live audio source/device")]
    InvalidAudioSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProductionSnapshot {
    pub revision: u64,
    pub program: Option<InputId>,
    pub preview: Option<InputId>,
    pub overlay1: Option<InputId>,
    #[serde(default)]
    pub overlays: [Option<InputId>; 4],
    pub master_volume: f32,
    pub master_muted: bool,
}

impl ProductionSnapshot {
    pub fn from_state(revision: u64, mixer: &MixerState) -> Self {
        Self {
            revision,
            program: mixer.program_input,
            preview: mixer.preview_input,
            overlay1: mixer.active_overlay_1,
            overlays: mixer.active_overlays,
            master_volume: mixer.master_bus.volume,
            master_muted: mixer.master_bus.muted,
        }
    }
}

/// Dispatches commands sequentially. Callers should own one serialized
/// dispatcher for each production, not create one per MIDI/API event.
#[derive(Debug, Default)]
pub struct CommandDispatcher {
    revision: u64,
}

impl CommandDispatcher {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Validate everything before changing state. All successful commands
    /// increment the revision so subscribers can reconcile device feedback.
    pub fn dispatch(
        &mut self,
        state: &mut MixerState,
        command: ProductionCommand,
    ) -> Result<ProductionSnapshot, CommandError> {
        match command {
            ProductionCommand::StartTestRecording { path } => {
                if path.trim().is_empty() || path.contains('\0') {
                    return Err(CommandError::InvalidRecordingPath);
                }
                state.recording_intent = Some(crate::RecordingIntent {
                    revision: self.revision.wrapping_add(1),
                    path: Some(path),
                    live_audio: None,
                });
            }
            ProductionCommand::StartLiveRecording {
                path,
                factory,
                device,
            } => {
                if path.trim().is_empty() || path.contains('\0') {
                    return Err(CommandError::InvalidRecordingPath);
                }
                if factory.is_empty()
                    || !factory
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_')
                    || device.as_ref().is_some_and(|d| d.contains('\0'))
                {
                    return Err(CommandError::InvalidAudioSource);
                }
                state.recording_intent = Some(crate::RecordingIntent {
                    revision: self.revision.wrapping_add(1),
                    path: Some(path),
                    live_audio: Some((factory, device)),
                });
            }
            ProductionCommand::StopRecording => {
                state.recording_intent = Some(crate::RecordingIntent {
                    revision: self.revision.wrapping_add(1),
                    path: None,
                    live_audio: None,
                });
            }
            ProductionCommand::SelectPreview { input } => {
                require_input(state, input)?;
                state.select_preview(input);
            }
            ProductionCommand::SelectProgram { input } => {
                require_input(state, input)?;
                state.select_program(input);
            }
            ProductionCommand::Cut => {
                let input = state.preview_input.ok_or(CommandError::NoPreview)?;
                require_input(state, input)?;
                state.cut();
            }
            ProductionCommand::ToggleOverlay { channel, input } => {
                if !(1..=4).contains(&channel) {
                    return Err(CommandError::InvalidOverlayChannel(channel));
                }
                require_input(state, input)?;
                let _ = state.toggle_overlay(channel, input);
            }
            ProductionCommand::SetOverlay { channel, input } => {
                if !(1..=4).contains(&channel) {
                    return Err(CommandError::InvalidOverlayChannel(channel));
                }
                if let Some(input) = input {
                    require_input(state, input)?;
                }
                let _ = state.set_overlay(channel, input);
            }
            ProductionCommand::ToggleOverlay1 { input } => {
                require_input(state, input)?;
                state.toggle_overlay_1(input);
            }
            ProductionCommand::SetOverlay1 { input } => {
                if let Some(input) = input {
                    require_input(state, input)?;
                }
                let _ = state.set_overlay(1, input);
            }
            ProductionCommand::SetInputVolume { input, volume } => {
                check_volume(volume)?;
                let source = state
                    .inputs
                    .iter_mut()
                    .find(|item| item.id == input)
                    .ok_or(CommandError::UnknownInput(input))?;
                source.volume = volume;
            }
            ProductionCommand::SetInputMute { input, muted } => {
                let source = state
                    .inputs
                    .iter_mut()
                    .find(|item| item.id == input)
                    .ok_or(CommandError::UnknownInput(input))?;
                source.is_muted = muted;
            }
            ProductionCommand::SetInputAfv { input, enabled } => {
                let source = state
                    .inputs
                    .iter_mut()
                    .find(|item| item.id == input)
                    .ok_or(CommandError::UnknownInput(input))?;
                source.afv = enabled;
            }
            ProductionCommand::SetMasterVolume { volume } => {
                check_volume(volume)?;
                state.master_bus.volume = volume;
            }
            ProductionCommand::SetMasterMute { muted } => {
                state.master_bus.muted = muted;
            }
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(ProductionSnapshot::from_state(self.revision, state))
    }
}

fn require_input(state: &MixerState, input: InputId) -> Result<(), CommandError> {
    if state.inputs.iter().any(|item| item.id == input) {
        Ok(())
    } else {
        Err(CommandError::UnknownInput(input))
    }
}

fn check_volume(value: f32) -> Result<(), CommandError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        Err(CommandError::InvalidVolume)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_cut_tally_and_revisions_are_identical_for_all_callers() {
        let mut state = MixerState::default();
        let mut dispatcher = CommandDispatcher::default();
        dispatcher
            .dispatch(&mut state, ProductionCommand::SelectPreview { input: 2 })
            .unwrap();
        let result = dispatcher
            .dispatch(&mut state, ProductionCommand::Cut)
            .unwrap();
        assert_eq!(result.program, Some(2));
        assert_eq!(result.preview, Some(1));
        assert_eq!(result.revision, 2);
        assert!(
            state
                .inputs
                .iter()
                .find(|i| i.id == 2)
                .unwrap()
                .is_tally_program
        );
        assert!(
            state
                .inputs
                .iter()
                .find(|i| i.id == 1)
                .unwrap()
                .is_tally_preview
        );
    }

    #[test]
    fn invalid_input_does_not_change_anything() {
        let mut state = MixerState::default();
        let mut dispatcher = CommandDispatcher::default();
        let original = serde_json::to_value(&state).unwrap();
        assert_eq!(
            dispatcher.dispatch(&mut state, ProductionCommand::SelectProgram { input: 5000 }),
            Err(CommandError::UnknownInput(5000))
        );
        assert_eq!(serde_json::to_value(&state).unwrap(), original);
        assert_eq!(dispatcher.revision(), 0);
    }

    #[test]
    fn invalid_volume_is_rejected_atomically() {
        let mut state = MixerState::default();
        let mut dispatcher = CommandDispatcher::default();
        for volume in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert_eq!(
                dispatcher.dispatch(
                    &mut state,
                    ProductionCommand::SetInputVolume { input: 1, volume }
                ),
                Err(CommandError::InvalidVolume)
            );
        }
        assert_eq!(state.inputs[0].volume, 0.85);
        assert_eq!(dispatcher.revision(), 0);
    }

    #[test]
    fn overlay_toggle_and_master_mute_are_reflected_in_feedback() {
        let mut state = MixerState::default();
        let mut dispatcher = CommandDispatcher::default();
        let first = dispatcher
            .dispatch(&mut state, ProductionCommand::ToggleOverlay1 { input: 5 })
            .unwrap();
        assert_eq!(first.overlay1, Some(5));
        let second = dispatcher
            .dispatch(&mut state, ProductionCommand::SetMasterMute { muted: true })
            .unwrap();
        assert!(second.master_muted);
        let third = dispatcher
            .dispatch(&mut state, ProductionCommand::ToggleOverlay1 { input: 5 })
            .unwrap();
        assert_eq!(third.overlay1, None);
    }

    #[test]
    fn explicit_overlay_commands_are_idempotent() {
        let mut state = MixerState::default();
        let mut dispatcher = CommandDispatcher::default();
        for _ in 0..2 {
            dispatcher
                .dispatch(
                    &mut state,
                    ProductionCommand::SetOverlay1 { input: Some(5) },
                )
                .unwrap();
            assert_eq!(state.active_overlay_1, Some(5));
        }
        dispatcher
            .dispatch(&mut state, ProductionCommand::SetOverlay1 { input: None })
            .unwrap();
        assert_eq!(state.active_overlay_1, None);
        let snapshot = serde_json::to_value(&state).unwrap();
        assert_eq!(
            dispatcher.dispatch(
                &mut state,
                ProductionCommand::SetOverlay1 { input: Some(999) }
            ),
            Err(CommandError::UnknownInput(999))
        );
        assert_eq!(serde_json::to_value(&state).unwrap(), snapshot);
    }

    #[test]
    fn serialization_uses_stable_action_names() {
        let command = ProductionCommand::SelectPreview { input: 3 };
        let encoded = serde_json::to_string(&command).unwrap();
        assert_eq!(encoded, r#"{"action":"select_preview","args":{"input":3}}"#);
        let decoded: ProductionCommand = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, command);
    }

    #[test]
    fn four_overlays_toggle_independently_and_persist_in_snapshot() {
        let mut state = MixerState::default();
        let mut dispatcher = CommandDispatcher::default();

        // Toggle OV1 with input 1, OV2 with input 2, OV3 with input 3, OV4 with input 4
        dispatcher
            .dispatch(
                &mut state,
                ProductionCommand::ToggleOverlay { channel: 1, input: 1 },
            )
            .unwrap();
        dispatcher
            .dispatch(
                &mut state,
                ProductionCommand::ToggleOverlay { channel: 2, input: 2 },
            )
            .unwrap();
        dispatcher
            .dispatch(
                &mut state,
                ProductionCommand::ToggleOverlay { channel: 3, input: 3 },
            )
            .unwrap();
        let snapshot = dispatcher
            .dispatch(
                &mut state,
                ProductionCommand::ToggleOverlay { channel: 4, input: 4 },
            )
            .unwrap();

        assert_eq!(snapshot.overlay1, Some(1));
        assert_eq!(snapshot.overlays, [Some(1), Some(2), Some(3), Some(4)]);

        // Cut background should preserve all active overlays
        let cut_snapshot = dispatcher
            .dispatch(&mut state, ProductionCommand::Cut)
            .unwrap();
        assert_eq!(cut_snapshot.overlays, [Some(1), Some(2), Some(3), Some(4)]);

        // Removing input 2 clears only overlay channel 2
        state.remove_input(2);
        assert_eq!(state.active_overlays, [Some(1), None, Some(3), Some(4)]);

        // Invalid channels are rejected
        assert_eq!(
            dispatcher.dispatch(
                &mut state,
                ProductionCommand::ToggleOverlay { channel: 0, input: 1 }
            ),
            Err(CommandError::InvalidOverlayChannel(0))
        );
        assert_eq!(
            dispatcher.dispatch(
                &mut state,
                ProductionCommand::ToggleOverlay { channel: 5, input: 1 }
            ),
            Err(CommandError::InvalidOverlayChannel(5))
        );
    }
}

#[cfg(test)]
mod recording_intent_tests {
    use super::*;
    #[test]
    fn recording_requests_share_dispatcher_but_do_not_claim_actual_recording() {
        let mut state = MixerState::default();
        let mut dispatcher = CommandDispatcher::default();
        assert_eq!(
            dispatcher.dispatch(
                &mut state,
                ProductionCommand::StartTestRecording {
                    path: String::new()
                }
            ),
            Err(CommandError::InvalidRecordingPath)
        );
        assert_eq!(dispatcher.revision(), 0);
        assert!(state.recording_intent.is_none());
        dispatcher
            .dispatch(
                &mut state,
                ProductionCommand::StartTestRecording {
                    path: "recordings/test.webm".into(),
                },
            )
            .unwrap();
        assert_eq!(state.recording_intent.as_ref().unwrap().revision, 1);
        assert!(!state.is_recording);
        let restored: MixerState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert!(
            restored.recording_intent.is_none(),
            "Opening a saved state must never replay a file-write request"
        );
        dispatcher
            .dispatch(&mut state, ProductionCommand::StopRecording)
            .unwrap();
        assert!(state.recording_intent.as_ref().unwrap().path.is_none());
        assert_eq!(state.recording_intent.as_ref().unwrap().revision, 2);
    }
}

#[cfg(test)]
mod live_recording_tests {
    use super::*;
    #[test]
    fn live_intents_remain_explicit_and_validate_before_mutation() {
        let mut state = MixerState::default();
        let mut dispatcher = CommandDispatcher::default();
        let invalid = ProductionCommand::StartLiveRecording {
            path: "file.webm".into(),
            factory: "pulsesrc ! fakesink".into(),
            device: None,
        };
        assert_eq!(
            dispatcher.dispatch(&mut state, invalid),
            Err(CommandError::InvalidAudioSource)
        );
        assert_eq!(dispatcher.revision(), 0);
        assert!(state.recording_intent.is_none());
        let command = ProductionCommand::StartLiveRecording {
            path: "file.webm".into(),
            factory: "pulsesrc".into(),
            device: Some("mic-1".into()),
        };
        let json = serde_json::to_string(&command).unwrap();
        let decoded: ProductionCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, command);
        dispatcher.dispatch(&mut state, command).unwrap();
        assert_eq!(
            state.recording_intent.as_ref().unwrap().live_audio,
            Some(("pulsesrc".into(), Some("mic-1".into())))
        );
        dispatcher
            .dispatch(&mut state, ProductionCommand::StopRecording)
            .unwrap();
        assert!(state.recording_intent.as_ref().unwrap().path.is_none());
        dispatcher
            .dispatch(
                &mut state,
                ProductionCommand::StartTestRecording {
                    path: "tone.webm".into(),
                },
            )
            .unwrap();
        assert!(state
            .recording_intent
            .as_ref()
            .unwrap()
            .live_audio
            .is_none());
    }
}
