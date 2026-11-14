//! Native MIDI ingress. Port enumeration and callbacks use midir, while all
//! bindings and production effects remain in the platform-neutral core.
//! No MIDI callback may directly mutate mixer state or block audio/video.

use midir::{Ignore, MidiInput, MidiInputConnection};
use openvideomix_core::midi::MidiEvent;
use std::sync::mpsc::{self, Receiver, TryRecvError};

const MAX_PENDING_EVENTS: usize = 256;

pub struct MidiInputService {
    _connection: MidiInputConnection<()>,
    receiver: Receiver<MidiEvent>,
    pub port_name: String,
}

impl MidiInputService {
    pub fn ports() -> Result<Vec<String>, String> {
        let midi = MidiInput::new("OPENVIDEOMIX ports")
            .map_err(|err| format!("MIDI initialization failed: {err}"))?;
        midi.ports()
            .iter()
            .map(|port| {
                midi.port_name(port)
                    .map_err(|err| format!("MIDI port name failed: {err}"))
            })
            .collect()
    }

    pub fn connect(port_index: usize) -> Result<Self, String> {
        let mut midi = MidiInput::new("OPENVIDEOMIX MIDI input")
            .map_err(|err| format!("MIDI initialization failed: {err}"))?;
        midi.ignore(Ignore::None);
        let ports = midi.ports();
        let port = ports
            .get(port_index)
            .ok_or_else(|| format!("MIDI port index {port_index} is not available"))?;
        let port_name = midi
            .port_name(port)
            .map_err(|err| format!("MIDI port name failed: {err}"))?;
        let (sender, receiver) = mpsc::sync_channel(MAX_PENDING_EVENTS);
        let connection = midi
            .connect(
                port,
                "OPENVIDEOMIX control",
                move |_timestamp, data, _state| {
                    if let Some(event) = MidiEvent::parse(data) {
                        // Ignore overflow rather than blocking the system MIDI callback.
                        let _ = sender.try_send(event);
                    }
                },
                (),
            )
            .map_err(|err| format!("MIDI connection failed: {err}"))?;
        Ok(Self {
            _connection: connection,
            receiver,
            port_name,
        })
    }

    pub fn drain(&self) -> Vec<MidiEvent> {
        let mut events = Vec::new();
        for _ in 0..MAX_PENDING_EVENTS {
            match self.receiver.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        events
    }
}
