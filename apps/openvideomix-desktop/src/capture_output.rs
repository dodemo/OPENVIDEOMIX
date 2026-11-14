//! Capture ingress for the CPU reference Program output, independent of egui.
//! Requires shared-clock PTS; it never substitutes a source-local timestamp.
//! This is not a GPU engine, scheduler, resampler or audio synchronizer.

use crate::capture::CameraFrame;
use openvideomix_core::{
    reference_compositor::{CompositeError, RgbaFrame},
    reference_output::{FrameQueueError, ReferenceOutput, ReferenceTake, RenderError, TimedFrame},
    video_profile::VideoProfile,
    InputId,
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngressError {
    InvalidProfile,
    DuplicateInput(InputId),
    UnknownInput(InputId),
    MissingSharedTimestamp,
    /// Explicit source conversion/configuration is required; no implicit scale.
    DimensionMismatch,
    InvalidPixels(CompositeError),
    Queue(FrameQueueError),
    Output(RenderError),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct IngressStats {
    pub accepted_frames: u64,
    pub rejected_frames: u64,
    pub queue_overflow_frames: u64,
    pub last_source_pts_ns: Option<u64>,
    pub last_capture_pts_ns: Option<u64>,
}

/// An output worker can own this adapter and supply deadlines in the shared
/// capture epoch. Registration, ingress and rendering do not access the GUI.
/// Output deadlines are provided by the caller; this does not run a live clock.
#[derive(Default)]
pub struct CaptureOutput {
    profile: VideoProfile,
    output: ReferenceOutput,
    stats: HashMap<InputId, IngressStats>,
}

impl CaptureOutput {
    /// Alternate profiles must be selected explicitly. Native source rates are
    /// not inferred from requested output cadence or buffer durations.
    pub fn new(profile: VideoProfile) -> Result<Self, IngressError> {
        if profile.width == 0
            || profile.height == 0
            || profile.frame_rate.numerator == 0
            || profile.frame_rate.denominator == 0
        {
            return Err(IngressError::InvalidProfile);
        }
        Ok(Self {
            profile,
            ..Self::default()
        })
    }

    pub fn profile(&self) -> VideoProfile {
        self.profile
    }

    pub fn add_source(&mut self, input: InputId, capacity: usize) -> Result<(), IngressError> {
        if self.stats.contains_key(&input) {
            return Err(IngressError::DuplicateInput(input));
        }
        self.output
            .add_source(input, capacity)
            .map_err(IngressError::Queue)?;
        self.stats.insert(input, IngressStats::default());
        Ok(())
    }

    pub fn remove_source(&mut self, input: InputId) {
        self.output.remove_source(input);
        self.stats.remove(&input);
    }

    /// Validates owned packed pixels and preserves the original source PTS as
    /// metadata. Invalid/late input leaves queued output frames unchanged.
    pub fn ingest(&mut self, input: InputId, frame: CameraFrame) -> Result<(), IngressError> {
        let stats = self
            .stats
            .get_mut(&input)
            .ok_or(IngressError::UnknownInput(input))?;
        let local_pts = frame.pts_ns;
        let shared_pts = frame.capture_pts_ns;
        let result = (|| {
            let pts_ns = shared_pts.ok_or(IngressError::MissingSharedTimestamp)?;
            if frame.width != self.profile.width as usize
                || frame.height != self.profile.height as usize
            {
                return Err(IngressError::DimensionMismatch);
            }
            let rgba = RgbaFrame::new(self.profile.width, self.profile.height, frame.rgba)
                .map_err(IngressError::InvalidPixels)?;
            self.output
                .push(input, TimedFrame { pts_ns, rgba })
                .map_err(IngressError::Output)
        })();
        if result.is_ok() {
            stats.accepted_frames += 1;
            stats.last_source_pts_ns = local_pts;
            stats.last_capture_pts_ns = shared_pts;
            stats.queue_overflow_frames = self.output.source_dropped_frames(input).unwrap_or(0);
        } else {
            stats.rejected_frames += 1;
        }
        result
    }

    pub fn stats(&self, input: InputId) -> Option<&IngressStats> {
        self.stats.get(&input)
    }

    pub fn source_selected_pts_ns(&self, input: InputId) -> Option<u64> {
        self.output.source_selected_pts_ns(input)
    }

    /// Compose Program and OV1–OV4 at a supplied common-clock deadline.
    /// No future frame leaks into output. Missing sources are explicit errors.
    pub fn render_at(
        &mut self,
        deadline_ns: u64,
        program: InputId,
        take: Option<ReferenceTake>,
        overlays: [Option<InputId>; 4],
    ) -> Result<RgbaFrame, IngressError> {
        self.output
            .render_at(deadline_ns, program, take, overlays)
            .map_err(IngressError::Output)
    }
}
