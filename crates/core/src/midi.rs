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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    SelectPreview(InputId),
    SelectProgram(InputId),
    ToggleOverlay1(InputId),
    MasterVolume,
    InputVolume(InputId),
}

/// Mapping is saved with a logical device profile by the host application.
/// CC volume mapping is absolute (0-127); relative encoders are future scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiBinding {
    pub source: MidiSource,
    pub target: MidiTarget,
}

/// State changes must be dispatched by CommandDispatcher; this adapter only
/// translates complete MIDI events into typed commands. Note actions fire on
/// NoteOn, not NoteOff; CC button actions fire on the 0 -> nonzero edge.
#[derive(Debug, Default)]
pub struct MidiMapper {
    pub bindings: Vec<MidiBinding>,
    last_cc_values: Vec<Option<u8>>,
    picked_up: Vec<bool>,
    last_sent_volumes: Vec<Option<f32>>,
}

impl MidiMapper {
    pub fn new(bindings: Vec<MidiBinding>) -> Self {
        let count = bindings.len();
        Self {
            bindings,
            last_cc_values: vec![None; count],
            picked_up: vec![false; count],
            last_sent_volumes: vec![None; count],
        }
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
                    if let Some(command) = trigger(&mapping.target) {
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
                        if let Some(command) = trigger(&mapping.target) {
                            commands.push(command);
                        }
                    }
                }
            }
        }
        commands
    }
}

fn trigger(target: &MidiTarget) -> Option<ProductionCommand> {
    match *target {
        MidiTarget::Cut => Some(ProductionCommand::Cut),
        MidiTarget::SelectPreview(input) => Some(ProductionCommand::SelectPreview { input }),
        MidiTarget::SelectProgram(input) => Some(ProductionCommand::SelectProgram { input }),
        MidiTarget::ToggleOverlay1(input) => Some(ProductionCommand::ToggleOverlay1 { input }),
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
}
