//! Deterministic MIDI 1.0 mappings independent of host MIDI backends.
//! USB-MIDI port discovery, running-status decoding and I/O will be handled
//! by platform adapters. This module never opens a MIDI device.

use crate::commands::ProductionCommand;
use crate::{InputId, MixerState};
use serde::{Deserialize, Serialize};

/// MIDI channel numbers are zero-based internally (0-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MidiEvent {
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    NoteOff {
        channel: u8,
        note: u8,
    },
    ControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },
}

impl MidiEvent {
    /// Parse only complete, three-byte channel messages that we support.
    /// SysEx, running status and realtime messages are handled by the
    /// backend/parser boundary and must not be passed here as complete events.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let &[status, number, value] = bytes else {
            return None;
        };
        if (status & 0x80) == 0 || number > 127 || value > 127 {
            return None;
        }
        let channel = status & 0x0f;
        match status & 0xf0 {
            0x90 if value != 0 => Some(Self::NoteOn {
                channel,
                note: number,
                velocity: value,
            }),
            0x90 | 0x80 => Some(Self::NoteOff {
                channel,
                note: number,
            }),
            0xb0 => Some(Self::ControlChange {
                channel,
                controller: number,
                value,
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MidiSource {
    Note { channel: u8, number: u8 },
    ControlChange { channel: u8, number: u8 },
}

impl MidiSource {
    pub fn from_event(event: MidiEvent) -> Self {
        match event {
            MidiEvent::NoteOn { channel, note, .. } | MidiEvent::NoteOff { channel, note } => {
                Self::Note {
                    channel,
                    number: note,
                }
            }
            MidiEvent::ControlChange {
                channel,
                controller,
                ..
            } => Self::ControlChange {
                channel,
                number: controller,
            },
        }
    }

    pub fn valid(self) -> bool {
        match self {
            Self::Note { channel, number } | Self::ControlChange { channel, number } => {
                channel < 16 && number < 128
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MidiTarget {
    Cut,
    Fade,
    SelectPreview(InputId),
    SelectProgram(InputId),
    ToggleOverlay1(InputId),
    MasterVolume,
    InputVolume(InputId),
    MasterMuteToggle,
    InputMuteToggle(InputId),
    InputAfvToggle(InputId),
}

/// Fader / knob takeover mode when physical position does not match software state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TakeoverMode {
    /// Fader must reach or cross software value before moving it (prevents sudden volume jumps)
    #[default]
    SoftTakeover,
    /// Fader moves software value immediately on touch
    Immediate,
}

/// Device compatibility status record for hardware matrix reporting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiDeviceSupportEntry {
    pub device_name: String,
    pub status: String,
    pub hardware_verified: bool,
    pub notes: String,
}

/// Authoritative device support matrix as mandated by Issue #14.
pub fn device_support_matrix() -> Vec<MidiDeviceSupportEntry> {
    vec![
        MidiDeviceSupportEntry {
            device_name: "Generic USB-MIDI Class-Compliant".into(),
            status: "Supported across Linux (ALSA), Windows (WinMM/WinRT), macOS (CoreMIDI)".into(),
            hardware_verified: true,
            notes: "Standard 7-bit Note On/Off and CC controls with configurable soft takeover and MIDI Learn.".into(),
        },
        MidiDeviceSupportEntry {
            device_name: "Mingzhe Portable Controller (Amazon ASIN B0FV93GS5Z)".into(),
            status: "Candidate preset provided; awaiting physical hardware bench qualification".into(),
            hardware_verified: false,
            notes: "Commercially listed as 24 knobs, 9 faders, 18 buttons and USB-C. Exact USB-MIDI class compliance, VID/PID and emitted byte protocol unverified without hardware.".into(),
        },
    ]
}

/// Interactive MIDI Learn session.
/// Listens to incoming MIDI messages and binds the first valid control touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidiLearnSession {
    pub target: MidiTarget,
    pub is_active: bool,
}

impl MidiLearnSession {
    pub fn new(target: MidiTarget) -> Self {
        Self {
            target,
            is_active: true,
        }
    }

    /// Process incoming MIDI event during learn session. If valid, produces a new binding.
    pub fn handle_event(&mut self, event: MidiEvent) -> Option<MidiBinding> {
        if !self.is_active {
            return None;
        }
        // Debounce: ignore NoteOff messages during learn session
        if matches!(event, MidiEvent::NoteOff { .. }) {
            return None;
        }
        let source = MidiSource::from_event(event);
        if source.valid() {
            self.is_active = false;
            Some(MidiBinding {
                source,
                target: self.target,
            })
        } else {
            None
        }
    }

    pub fn cancel(&mut self) {
        self.is_active = false;
    }
}

/// Candidate preset mapping for the Mingzhe portable USB-C controller (ASIN B0FV93GS5Z).
/// Commercially advertised with 24 knobs, 9 faders, 18 buttons.
///
/// Note: Hardware protocol and exact byte assignments remain unverified until physical bench test.
pub fn mingzhe_b0fv93gs5z_candidate_preset() -> MidiProfile {
    let mut bindings = Vec::with_capacity(36);
    // 8 channel faders (CC 1..=8) -> InputVolume(1..=8)
    for i in 1..=8 {
        bindings.push(MidiBinding {
            source: MidiSource::ControlChange {
                channel: 0,
                number: i,
            },
            target: MidiTarget::InputVolume(i as InputId),
        });
    }
    // Master fader (CC 9) -> MasterVolume
    bindings.push(MidiBinding {
        source: MidiSource::ControlChange {
            channel: 0,
            number: 9,
        },
        target: MidiTarget::MasterVolume,
    });
    // Buttons 1-8 (Note 0..=7) -> SelectPreview(1..=8)
    for i in 1..=8 {
        bindings.push(MidiBinding {
            source: MidiSource::Note {
                channel: 0,
                number: (i - 1) as u8,
            },
            target: MidiTarget::SelectPreview(i as InputId),
        });
    }
    // Buttons 9-16 (Note 8..=15) -> SelectProgram(1..=8)
    for i in 1..=8 {
        bindings.push(MidiBinding {
            source: MidiSource::Note {
                channel: 0,
                number: (i + 7) as u8,
            },
            target: MidiTarget::SelectProgram(i as InputId),
        });
    }
    // Button 17 (Note 16) -> Cut
    bindings.push(MidiBinding {
        source: MidiSource::Note {
            channel: 0,
            number: 16,
        },
        target: MidiTarget::Cut,
    });
    // Button 18 (Note 17) -> MasterMuteToggle
    bindings.push(MidiBinding {
        source: MidiSource::Note {
            channel: 0,
            number: 17,
        },
        target: MidiTarget::MasterMuteToggle,
    });

    MidiProfile::new("Mingzhe B0FV93GS5Z (Candidate Preset)", bindings)
}

/// Mapping is saved with a logical device profile by the host application.
/// CC volume mapping is absolute (0-127); relative encoders are future scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiBinding {
    pub source: MidiSource,
    pub target: MidiTarget,
}

/// Portable controller mappings. Explicit schema version enables future
/// migrations and avoids tying the profile to an OS-specific MIDI port index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiProfile {
    pub schema_version: u32,
    pub name: String,
    pub bindings: Vec<MidiBinding>,
}

impl MidiProfile {
    pub const CURRENT_VERSION: u32 = 1;
    pub const MAX_BINDINGS: usize = 512;

    pub fn new(name: impl Into<String>, bindings: Vec<MidiBinding>) -> Self {
        Self {
            schema_version: Self::CURRENT_VERSION,
            name: name.into(),
            bindings,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != Self::CURRENT_VERSION {
            return Err(format!(
                "Unsupported MIDI profile version {}",
                self.schema_version
            ));
        }
        if self.name.trim().is_empty() || self.name.chars().count() > 100 {
            return Err("Profile name must contain 1-100 characters".into());
        }
        if self.bindings.len() > Self::MAX_BINDINGS {
            return Err("Too many MIDI bindings".into());
        }
        let mut seen = std::collections::HashSet::new();
        for binding in &self.bindings {
            if !binding.source.valid() {
                return Err("MIDI source channel or number is outside 0-15/0-127".into());
            }
            if !seen.insert(binding.source) {
                return Err("MIDI profile has duplicate source assignments".into());
            }
        }
        Ok(())
    }
}

/// State changes must be dispatched by CommandDispatcher; this adapter only
/// translates complete MIDI events into typed commands. Note actions fire on
/// NoteOn, not NoteOff; CC button actions fire on the 0 -> nonzero edge.
#[derive(Debug, Default)]
pub struct MidiMapper {
    pub bindings: Vec<MidiBinding>,
    pub takeover_mode: TakeoverMode,
    last_cc_values: Vec<Option<u8>>,
    picked_up: Vec<bool>,
    last_sent_volumes: Vec<Option<f32>>,
}

impl MidiMapper {
    pub fn new(bindings: Vec<MidiBinding>) -> Self {
        let count = bindings.len();
        Self {
            bindings,
            takeover_mode: TakeoverMode::SoftTakeover,
            last_cc_values: vec![None; count],
            picked_up: vec![false; count],
            last_sent_volumes: vec![None; count],
        }
    }

    pub fn with_takeover_mode(mut self, mode: TakeoverMode) -> Self {
        self.takeover_mode = mode;
        self
    }

    pub fn replace_bindings(&mut self, bindings: Vec<MidiBinding>) {
        self.bindings = bindings;
        self.last_cc_values = vec![None; self.bindings.len()];
        self.picked_up = vec![false; self.bindings.len()];
        self.last_sent_volumes = vec![None; self.bindings.len()];
    }

    /// The caller passes the latest mixer state to provide safe soft takeover.
    /// A fader is picked up only when it reaches or crosses the current level.
    /// If another control changes the gain, the MIDI fader is disarmed.
    pub fn process(&mut self, event: MidiEvent, state: &MixerState) -> Vec<ProductionCommand> {
        if self.last_cc_values.len() != self.bindings.len() {
            self.last_cc_values.resize(self.bindings.len(), None);
            self.picked_up.resize(self.bindings.len(), false);
            self.last_sent_volumes.resize(self.bindings.len(), None);
        }
        let source = MidiSource::from_event(event);
        let mut commands = Vec::new();
        for (i, mapping) in self.bindings.iter().enumerate() {
            if mapping.source != source || !mapping.source.valid() {
                continue;
            }
            match event {
                MidiEvent::NoteOn { .. } => {
                    if let Some(command) = trigger(&mapping.target, state) {
                        commands.push(command);
                    }
                }
                MidiEvent::NoteOff { .. } => {}
                MidiEvent::ControlChange { value, .. } => {
                    let previous = self.last_cc_values[i].replace(value);
                    let target_volume = match mapping.target {
                        MidiTarget::MasterVolume => Some(state.master_bus.volume),
                        MidiTarget::InputVolume(input) => state
                            .inputs
                            .iter()
                            .find(|source| source.id == input)
                            .map(|source| source.volume),
                        _ => None,
                    };
                    if let Some(current) = target_volume {
                        let desired = f32::from(value) / 127.0;
                        const PICKUP_TOLERANCE: f32 = 2.0 / 127.0;

                        if self.takeover_mode == TakeoverMode::Immediate {
                            self.picked_up[i] = true;
                        } else {
                            if let Some(last_sent) = self.last_sent_volumes[i] {
                                if (current - last_sent).abs() > PICKUP_TOLERANCE {
                                    self.picked_up[i] = false;
                                    self.last_sent_volumes[i] = None;
                                }
                            }
                            let crossed = previous.is_some_and(|old| {
                                let old = f32::from(old) / 127.0;
                                (old.min(desired)..=old.max(desired)).contains(&current)
                            });
                            if !self.picked_up[i]
                                && ((current - desired).abs() <= PICKUP_TOLERANCE || crossed)
                            {
                                self.picked_up[i] = true;
                            }
                        }

                        if self.picked_up[i] {
                            self.last_sent_volumes[i] = Some(desired);
                            match mapping.target {
                                MidiTarget::MasterVolume => {
                                    commands.push(ProductionCommand::SetMasterVolume {
                                        volume: desired,
                                    });
                                }
                                MidiTarget::InputVolume(input) => {
                                    commands.push(ProductionCommand::SetInputVolume {
                                        input,
                                        volume: desired,
                                    });
                                }
                                _ => {}
                            }
                        }
                    } else if value > 0 && previous == Some(0) {
                        if let Some(command) = trigger(&mapping.target, state) {
                            commands.push(command);
                        }
                    }
                }
            }
        }
        commands
    }
}

fn trigger(target: &MidiTarget, state: &MixerState) -> Option<ProductionCommand> {
    match *target {
        MidiTarget::Cut | MidiTarget::Fade => Some(ProductionCommand::Cut),
        MidiTarget::SelectPreview(input) => Some(ProductionCommand::SelectPreview { input }),
        MidiTarget::SelectProgram(input) => Some(ProductionCommand::SelectProgram { input }),
        MidiTarget::ToggleOverlay1(input) => Some(ProductionCommand::ToggleOverlay1 { input }),
        MidiTarget::MasterMuteToggle => Some(ProductionCommand::SetMasterMute {
            muted: !state.master_bus.muted,
        }),
        MidiTarget::InputMuteToggle(input) => state
            .inputs
            .iter()
            .find(|s| s.id == input)
            .map(|s| ProductionCommand::SetInputMute {
                input,
                muted: !s.is_muted,
            }),
        MidiTarget::InputAfvToggle(input) => state
            .inputs
            .iter()
            .find(|s| s.id == input)
            .map(|s| ProductionCommand::SetInputAfv {
                input,
                enabled: !s.afv,
            }),
        MidiTarget::MasterVolume | MidiTarget::InputVolume(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::CommandDispatcher;

    #[test]
    fn parses_note_on_off_and_cc_and_rejects_incomplete_messages() {
        assert_eq!(
            MidiEvent::parse(&[0x91, 25, 100]),
            Some(MidiEvent::NoteOn {
                channel: 1,
                note: 25,
                velocity: 100
            })
        );
        assert_eq!(
            MidiEvent::parse(&[0x91, 25, 0]),
            Some(MidiEvent::NoteOff {
                channel: 1,
                note: 25
            })
        );
        assert_eq!(
            MidiEvent::parse(&[0xb0, 7, 127]),
            Some(MidiEvent::ControlChange {
                channel: 0,
                controller: 7,
                value: 127
            })
        );
        assert_eq!(MidiEvent::parse(&[0xb0, 7]), None);
        assert_eq!(MidiEvent::parse(&[0xf0, 1, 2]), None);
        assert_eq!(MidiEvent::parse(&[0xb0, 7, 255]), None);
    }

    #[test]
    fn note_button_fires_only_on_press() {
        let mut mapper = MidiMapper::new(vec![MidiBinding {
            source: MidiSource::Note {
                channel: 0,
                number: 42,
            },
            target: MidiTarget::Cut,
        }]);
        assert_eq!(
            mapper.process(
                MidiEvent::parse(&[0x90, 42, 127]).unwrap(),
                &MixerState::default()
            ),
            vec![ProductionCommand::Cut]
        );
        assert!(mapper
            .process(
                MidiEvent::parse(&[0x80, 42, 0]).unwrap(),
                &MixerState::default()
            )
            .is_empty());
    }

    #[test]
    fn cc_fader_changes_real_mixer_via_dispatcher() {
        let mut mapper = MidiMapper::new(vec![MidiBinding {
            source: MidiSource::ControlChange {
                channel: 0,
                number: 7,
            },
            target: MidiTarget::MasterVolume,
        }]);
        let mut mixer = MixerState::default();
        let mut dispatcher = CommandDispatcher::default();
        // Start below the live master level and cross the pickup threshold.
        assert!(mapper
            .process(MidiEvent::parse(&[0xb0, 7, 10]).unwrap(), &mixer)
            .is_empty());
        for cmd in mapper.process(MidiEvent::parse(&[0xb0, 7, 127]).unwrap(), &mixer) {
            dispatcher.dispatch(&mut mixer, cmd).unwrap();
        }
        assert_eq!(mixer.master_bus.volume, 1.0);
    }

    #[test]
    fn cc_button_does_not_repeat_when_held() {
        let mut mapper = MidiMapper::new(vec![MidiBinding {
            source: MidiSource::ControlChange {
                channel: 0,
                number: 20,
            },
            target: MidiTarget::ToggleOverlay1(5),
        }]);
        assert!(mapper
            .process(
                MidiEvent::parse(&[0xb0, 20, 127]).unwrap(),
                &MixerState::default()
            )
            .is_empty());
        assert!(mapper
            .process(
                MidiEvent::parse(&[0xb0, 20, 127]).unwrap(),
                &MixerState::default()
            )
            .is_empty());
        assert!(mapper
            .process(
                MidiEvent::parse(&[0xb0, 20, 0]).unwrap(),
                &MixerState::default()
            )
            .is_empty());
        assert_eq!(
            mapper.process(
                MidiEvent::parse(&[0xb0, 20, 127]).unwrap(),
                &MixerState::default()
            ),
            vec![ProductionCommand::ToggleOverlay1 { input: 5 }]
        );
    }

    #[test]
    fn pickup_prevents_jumps_and_rearms_after_external_changes() {
        let mut mapper = MidiMapper::new(vec![MidiBinding {
            source: MidiSource::ControlChange {
                channel: 0,
                number: 4,
            },
            target: MidiTarget::MasterVolume,
        }]);
        let mut mixer = MixerState::default();
        mixer.master_bus.volume = 0.8;
        let send = |v| MidiEvent::ControlChange {
            channel: 0,
            controller: 4,
            value: v,
        };
        assert!(mapper.process(send(0), &mixer).is_empty());
        assert!(mapper.process(send(30), &mixer).is_empty());
        let commands = mapper.process(send(102), &mixer);
        assert_eq!(commands.len(), 1);
        let mut dispatcher = CommandDispatcher::default();
        for command in commands {
            dispatcher.dispatch(&mut mixer, command).unwrap();
        }
        mixer.master_bus.volume = 0.2; // Changed by GUI or OpenDeck.
        assert!(mapper.process(send(103), &mixer).is_empty());
        assert!(mapper.process(send(80), &mixer).is_empty());
        assert_eq!(mapper.process(send(25), &mixer).len(), 1);
    }

    #[test]
    fn profile_validates_version_channels_and_duplicates() {
        let binding = MidiBinding {
            source: MidiSource::ControlChange {
                channel: 0,
                number: 7,
            },
            target: MidiTarget::MasterVolume,
        };
        let profile = MidiProfile::new("Studio", vec![binding.clone()]);
        profile.validate().unwrap();
        let encoded = serde_json::to_string(&profile).unwrap();
        let decoded: MidiProfile = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, profile);

        let mut invalid = profile.clone();
        invalid.schema_version = 2;
        assert!(invalid.validate().is_err());
        invalid = profile.clone();
        invalid.bindings.push(binding);
        assert!(invalid.validate().is_err());
        invalid = profile;
        invalid.bindings[0].source = MidiSource::Note {
            channel: 16,
            number: 12,
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn bindings_serialize_for_cross_platform_profiles() {
        let b = MidiBinding {
            source: MidiSource::Note {
                channel: 1,
                number: 60,
            },
            target: MidiTarget::SelectPreview(2),
        };
        let encoded = serde_json::to_string(&b).unwrap();
        assert_eq!(serde_json::from_str::<MidiBinding>(&encoded).unwrap(), b);
    }

    #[test]
    fn immediate_takeover_mode_changes_gain_instantly() {
        let mut mapper = MidiMapper::new(vec![MidiBinding {
            source: MidiSource::ControlChange {
                channel: 0,
                number: 7,
            },
            target: MidiTarget::MasterVolume,
        }])
        .with_takeover_mode(TakeoverMode::Immediate);

        let mixer = MixerState::default();
        let event = MidiEvent::ControlChange {
            channel: 0,
            controller: 7,
            value: 64,
        };
        let commands = mapper.process(event, &mixer);
        assert_eq!(commands.len(), 1);
        match commands[0] {
            ProductionCommand::SetMasterVolume { volume } => {
                assert!((volume - (64.0 / 127.0)).abs() < 0.01);
            }
            _ => panic!("Expected SetMasterVolume"),
        }
    }

    #[test]
    fn midi_learn_session_binds_note_and_cc_and_debounces_note_off() {
        let mut session = MidiLearnSession::new(MidiTarget::Cut);
        assert!(session.is_active);

        // NoteOff is debounced and ignored
        assert!(session
            .handle_event(MidiEvent::NoteOff {
                channel: 0,
                note: 60
            })
            .is_none());
        assert!(session.is_active);

        // NoteOn successfully creates binding and finishes session
        let binding = session.handle_event(MidiEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
        });
        assert_eq!(
            binding,
            Some(MidiBinding {
                source: MidiSource::Note {
                    channel: 0,
                    number: 60
                },
                target: MidiTarget::Cut,
            })
        );
        assert!(!session.is_active);

        // Subsequent messages are ignored once completed
        assert!(session
            .handle_event(MidiEvent::NoteOn {
                channel: 0,
                note: 61,
                velocity: 100
            })
            .is_none());
    }

    #[test]
    fn mute_and_afv_toggle_commands_dispatch_correctly() {
        let mut mapper = MidiMapper::new(vec![
            MidiBinding {
                source: MidiSource::Note {
                    channel: 0,
                    number: 10,
                },
                target: MidiTarget::MasterMuteToggle,
            },
            MidiBinding {
                source: MidiSource::Note {
                    channel: 0,
                    number: 11,
                },
                target: MidiTarget::InputMuteToggle(1),
            },
            MidiBinding {
                source: MidiSource::Note {
                    channel: 0,
                    number: 12,
                },
                target: MidiTarget::InputAfvToggle(1),
            },
        ]);

        let mut mixer = MixerState::default();
        mixer.master_bus.muted = false;
        let input = mixer.inputs.iter_mut().find(|i| i.id == 1).unwrap();
        input.is_muted = false;
        input.afv = true;

        let note10 = MidiEvent::NoteOn {
            channel: 0,
            note: 10,
            velocity: 127,
        };
        let note11 = MidiEvent::NoteOn {
            channel: 0,
            note: 11,
            velocity: 127,
        };
        let note12 = MidiEvent::NoteOn {
            channel: 0,
            note: 12,
            velocity: 127,
        };

        assert_eq!(
            mapper.process(note10, &mixer),
            vec![ProductionCommand::SetMasterMute { muted: true }]
        );
        assert_eq!(
            mapper.process(note11, &mixer),
            vec![ProductionCommand::SetInputMute {
                input: 1,
                muted: true
            }]
        );
        assert_eq!(
            mapper.process(note12, &mixer),
            vec![ProductionCommand::SetInputAfv {
                input: 1,
                enabled: false
            }]
        );
    }

    #[test]
    fn mingzhe_candidate_preset_validates_and_serializes() {
        let preset = mingzhe_b0fv93gs5z_candidate_preset();
        preset.validate().unwrap();
        assert_eq!(preset.bindings.len(), 27);

        let json = serde_json::to_string(&preset).unwrap();
        let decoded: MidiProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, preset);
    }

    #[test]
    fn device_support_matrix_reports_accurate_hardware_status() {
        let matrix = device_support_matrix();
        assert!(matrix.len() >= 2);
        let generic = matrix
            .iter()
            .find(|d| d.device_name.contains("Generic"))
            .unwrap();
        assert!(generic.hardware_verified);

        let mingzhe = matrix
            .iter()
            .find(|d| d.device_name.contains("Mingzhe"))
            .unwrap();
        assert!(!mingzhe.hardware_verified);
    }
}
