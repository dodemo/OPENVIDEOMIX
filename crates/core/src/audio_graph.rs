//! Real broadcast audio graph, routing matrix, dBFS metering, AFV and drift accounting.
//!
//! Provides deterministic 48 kHz PCM mixing across Master, Bus A, Bus B, and
//! Headphones (PFL / AFL), with honest Peak/RMS dBFS meter calculations.

use crate::InputId;
use serde::{Deserialize, Serialize};

/// Standard broadcast audio sample rate
pub const STANDARD_AUDIO_SAMPLE_RATE: u32 = 48_000;

/// Number of stereo audio sample frames per video frame at 50.00 FPS PAL
pub const SAMPLES_PER_FRAME_50FPS: usize = 960;

/// Quiet threshold in dBFS (silence)
pub const SILENCE_DBFS: f32 = -100.0;

/// Convert linear amplitude (0.0 ..= 1.0+) to dBFS.
/// 1.0 linear corresponds to 0.0 dBFS (full scale).
#[inline]
pub fn linear_to_dbfs(linear: f32) -> f32 {
    if linear <= 0.000_000_1 {
        SILENCE_DBFS
    } else {
        (20.0 * linear.log10()).max(SILENCE_DBFS)
    }
}

/// Convert dBFS to linear amplitude.
/// 0.0 dBFS corresponds to 1.0 linear amplitude.
#[inline]
pub fn dbfs_to_linear(dbfs: f32) -> f32 {
    if dbfs <= SILENCE_DBFS {
        0.0
    } else {
        10.0_f32.powf(dbfs / 20.0)
    }
}

/// Real-time audio meter measurements in dBFS
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AudioMeterState {
    pub peak_dbfs: f32,
    pub rms_dbfs: f32,
    pub is_clipping: bool,
}

impl Default for AudioMeterState {
    fn default() -> Self {
        Self {
            peak_dbfs: SILENCE_DBFS,
            rms_dbfs: SILENCE_DBFS,
            is_clipping: false,
        }
    }
}

impl AudioMeterState {
    /// Calculate true peak and RMS dBFS from a slice of PCM f32 samples (-1.0 ..= 1.0)
    pub fn calculate(samples: &[f32]) -> Self {
        if samples.is_empty() {
            return Self::default();
        }

        let mut peak: f32 = 0.0;
        let mut sum_squares: f64 = 0.0;

        for &s in samples {
            let abs = s.abs();
            if abs > peak {
                peak = abs;
            }
            sum_squares += (f64::from(s)) * (f64::from(s));
        }

        let rms = (sum_squares / (samples.len() as f64)).sqrt() as f32;
        let peak_dbfs = linear_to_dbfs(peak);
        let rms_dbfs = linear_to_dbfs(rms);
        let is_clipping = peak >= 0.999 || peak_dbfs >= -0.05;

        Self {
            peak_dbfs,
            rms_dbfs,
            is_clipping,
        }
    }
}

/// Per-input audio routing configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelRouting {
    pub input_id: InputId,
    pub gain: f32, // Linear gain factor (1.0 = unity, 0.0 = muted)
    pub is_muted: bool,
    pub is_solo: bool,
    pub afv: bool, // Audio Follows Video: active only when input is on-air in Program
    pub send_master: bool,
    pub send_bus_a: bool,
    pub send_bus_b: bool,
    pub pfl: bool, // Pre-Fader Listen to headphones bus
    pub afl: bool, // After-Fader Listen to headphones bus
}

impl ChannelRouting {
    pub fn new(input_id: InputId) -> Self {
        Self {
            input_id,
            gain: 1.0,
            is_muted: false,
            is_solo: false,
            afv: true,
            send_master: true,
            send_bus_a: false,
            send_bus_b: false,
            pfl: false,
            afl: false,
        }
    }

    /// Effective gain for the main buses, taking into account mute, solo state and AFV.
    pub fn effective_bus_gain(&self, is_program: bool, any_solo_active: bool) -> f32 {
        if self.is_muted {
            return 0.0;
        }
        if any_solo_active && !self.is_solo {
            return 0.0;
        }
        if self.afv && !is_program {
            return 0.0;
        }
        self.gain.max(0.0)
    }

    /// Effective gain for Headphones PFL (Pre-Fader Listen, ignores fader gain and AFV).
    pub fn effective_pfl_gain(&self) -> f32 {
        if self.pfl && !self.is_muted {
            1.0
        } else {
            0.0
        }
    }

    /// Effective gain for Headphones AFL (After-Fader Listen).
    pub fn effective_afl_gain(&self) -> f32 {
        if self.afl && !self.is_muted {
            self.gain.max(0.0)
        } else {
            0.0
        }
    }
}

/// Broadcast bus summing output and meters
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioBusStrip {
    pub name: String,
    pub master_gain: f32, // Linear gain (1.0 = unity)
    pub is_muted: bool,
    pub meter_l: AudioMeterState,
    pub meter_r: AudioMeterState,
}

impl AudioBusStrip {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            master_gain: 1.0,
            is_muted: false,
            meter_l: AudioMeterState::default(),
            meter_r: AudioMeterState::default(),
        }
    }

    /// Calculate and update True Peak and RMS dBFS meters from stereo interleaved PCM
    pub fn update_meters(&mut self, stereo_samples: &[f32]) {
        if stereo_samples.len() < 2 {
            return;
        }

        let (chunks, _) = stereo_samples.as_chunks::<2>();
        let mut left_samples = Vec::with_capacity(chunks.len());
        let mut right_samples = Vec::with_capacity(chunks.len());

        for &[l, r] in chunks {
            left_samples.push(l);
            right_samples.push(r);
        }

        self.meter_l = AudioMeterState::calculate(&left_samples);
        self.meter_r = AudioMeterState::calculate(&right_samples);
    }
}

/// Statistics for audio/video clock alignment and drift
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AudioDriftStats {
    pub total_video_frames: u64,
    pub total_audio_samples: u64,
    pub expected_audio_samples: u64,
    pub drift_samples: i64,
    pub corrections_applied: u64,
}

/// Complete multi-channel broadcast AudioGraph
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioGraph {
    pub master_bus: AudioBusStrip,
    pub bus_a: AudioBusStrip,
    pub bus_b: AudioBusStrip,
    pub headphones_bus: AudioBusStrip,
    pub routings: Vec<ChannelRouting>,
    pub drift_stats: AudioDriftStats,
}

impl Default for AudioGraph {
    fn default() -> Self {
        Self {
            master_bus: AudioBusStrip::new("Master"),
            bus_a: AudioBusStrip::new("Bus A"),
            bus_b: AudioBusStrip::new("Bus B"),
            headphones_bus: AudioBusStrip::new("Headphones (PFL/AFL)"),
            routings: Vec::new(),
            drift_stats: AudioDriftStats::default(),
        }
    }
}

impl AudioGraph {
    /// Ensure a routing strip exists for an input
    pub fn ensure_input(&mut self, input_id: InputId) {
        if !self.routings.iter().any(|r| r.input_id == input_id) {
            self.routings.push(ChannelRouting::new(input_id));
        }
    }

    /// Process a block of multi-channel stereo interleaved PCM (L, R) for all inputs
    /// and sum onto Master, Bus A, Bus B and Headphones.
    ///
    /// Inputs is a list of tuples: `(InputId, &[f32] stereo_interleaved)`.
    /// `program_input` is the currently on-air InputId.
    pub fn mix(
        &mut self,
        inputs: &[(InputId, &[f32])],
        program_input: Option<InputId>,
    ) -> Vec<f32> {
        let sample_count = inputs.iter().map(|(_, s)| s.len()).max().unwrap_or(0);
        if sample_count == 0 {
            return Vec::new();
        }

        let any_solo = self.routings.iter().any(|r| r.is_solo);

        let mut master_sum = vec![0.0_f32; sample_count];
        let mut bus_a_sum = vec![0.0_f32; sample_count];
        let mut bus_b_sum = vec![0.0_f32; sample_count];
        let mut headphones_sum = vec![0.0_f32; sample_count];

        for &(id, samples) in inputs {
            self.ensure_input(id);
            let routing = self
                .routings
                .iter()
                .find(|r| r.input_id == id)
                .cloned()
                .unwrap();
            let is_pgm = program_input == Some(id);

            let bus_gain = routing.effective_bus_gain(is_pgm, any_solo);
            let pfl_gain = routing.effective_pfl_gain();
            let afl_gain = routing.effective_afl_gain();

            for (i, &sample) in samples.iter().enumerate() {
                if bus_gain > 0.0 {
                    let scaled = sample * bus_gain;
                    if routing.send_master {
                        master_sum[i] += scaled;
                    }
                    if routing.send_bus_a {
                        bus_a_sum[i] += scaled;
                    }
                    if routing.send_bus_b {
                        bus_b_sum[i] += scaled;
                    }
                }

                // Headphones monitoring: PFL or AFL
                if pfl_gain > 0.0 {
                    headphones_sum[i] += sample * pfl_gain;
                } else if afl_gain > 0.0 {
                    headphones_sum[i] += sample * afl_gain;
                }
            }
        }

        // Apply master bus fader and mute
        let m_gain = if self.master_bus.is_muted {
            0.0
        } else {
            self.master_bus.master_gain
        };
        for sample in &mut master_sum {
            *sample = (*sample * m_gain).clamp(-1.0, 1.0);
        }

        let a_gain = if self.bus_a.is_muted {
            0.0
        } else {
            self.bus_a.master_gain
        };
        for sample in &mut bus_a_sum {
            *sample = (*sample * a_gain).clamp(-1.0, 1.0);
        }

        let b_gain = if self.bus_b.is_muted {
            0.0
        } else {
            self.bus_b.master_gain
        };
        for sample in &mut bus_b_sum {
            *sample = (*sample * b_gain).clamp(-1.0, 1.0);
        }

        // Headphones default to Master if no PFL/AFL is selected
        let any_pfl_afl = self.routings.iter().any(|r| r.pfl || r.afl);
        if !any_pfl_afl {
            headphones_sum.copy_from_slice(&master_sum);
        }

        // Update real dBFS meters across all output strips
        self.master_bus.update_meters(&master_sum);
        self.bus_a.update_meters(&bus_a_sum);
        self.bus_b.update_meters(&bus_b_sum);
        self.headphones_bus.update_meters(&headphones_sum);

        master_sum
    }

    /// Account for audio samples vs video frame timestamps and record drift
    pub fn update_drift_accounting(
        &mut self,
        video_frames_elapsed: u64,
        audio_samples_received: u64,
    ) {
        self.drift_stats.total_video_frames = video_frames_elapsed;
        self.drift_stats.total_audio_samples = audio_samples_received;
        self.drift_stats.expected_audio_samples =
            video_frames_elapsed * (SAMPLES_PER_FRAME_50FPS as u64);
        self.drift_stats.drift_samples =
            (audio_samples_received as i64) - (self.drift_stats.expected_audio_samples as i64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dbfs_conversions() {
        assert_eq!(linear_to_dbfs(1.0), 0.0);
        assert!((linear_to_dbfs(0.5) - (-6.02)).abs() < 0.1);
        assert_eq!(linear_to_dbfs(0.0), SILENCE_DBFS);
        assert!((dbfs_to_linear(0.0) - 1.0).abs() < 0.001);
        assert!((dbfs_to_linear(-6.02) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_afv_routing() {
        let mut graph = AudioGraph::default();
        graph.ensure_input(1);
        graph.ensure_input(2);

        // Input 1 has AFV enabled (default), Input 2 has AFV disabled
        graph.routings[1].afv = false;

        // When Program is Input 1, Input 1 is on-air, Input 2 is also audible because AFV is off
        assert_eq!(graph.routings[0].effective_bus_gain(true, false), 1.0);
        assert_eq!(graph.routings[1].effective_bus_gain(false, false), 1.0);

        // When Program switches to Input 3, Input 1 goes silent due to AFV
        assert_eq!(graph.routings[0].effective_bus_gain(false, false), 0.0);
        assert_eq!(graph.routings[1].effective_bus_gain(false, false), 1.0);
    }

    #[test]
    fn test_pfl_isolation_from_master() {
        let mut graph = AudioGraph::default();
        graph.ensure_input(1);
        graph.routings[0].is_muted = false;
        graph.routings[0].pfl = true; // Solo headphones PFL

        let tone = vec![0.5_f32; 100]; // Stereo 50 samples
        let inputs = [(1, tone.as_slice())];

        // Mute master bus
        graph.master_bus.is_muted = true;
        let master_out = graph.mix(&inputs, Some(1));

        // Master output is silenced
        assert!(master_out.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_drift_accounting() {
        let mut graph = AudioGraph::default();
        // At 50 fps, 10 frames = 9600 samples
        graph.update_drift_accounting(10, 9600);
        assert_eq!(graph.drift_stats.drift_samples, 0);

        // 10 samples extra received
        graph.update_drift_accounting(10, 9610);
        assert_eq!(graph.drift_stats.drift_samples, 10);
    }
}
