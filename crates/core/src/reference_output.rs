//! Deterministic CPU-only output-frame selection for the reference compositor.
//!
//! Inputs must already share a comparable timeline. This module does NOT
//! synchronize independent GStreamer pipeline clocks, produce sound, encode
//! video, or measure live output FPS. It defines testable composition behavior
//! independently of any GUI, device, or wall-clock timer.

use std::collections::{HashMap, VecDeque};

use crate::reference_compositor::{
    overlay_stack, transition, CompositeError, RgbaFrame, Transition,
};
use crate::InputId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedFrame {
    pub pts_ns: u64,
    pub rgba: RgbaFrame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameQueueError {
    InvalidCapacity,
    NonMonotonicInput,
    /// The output has already passed this frame's timestamp.
    LateInput,
    NonMonotonicOutput,
}

#[derive(Debug)]
pub struct FrameQueue {
    capacity: usize,
    pending: VecDeque<TimedFrame>,
    last: Option<TimedFrame>,
    last_target_ns: Option<u64>,
    dropped: u64,
}

impl FrameQueue {
    pub fn new(capacity: usize) -> Result<Self, FrameQueueError> {
        if capacity == 0 {
            return Err(FrameQueueError::InvalidCapacity);
        }
        Ok(Self {
            capacity,
            pending: VecDeque::new(),
            last: None,
            last_target_ns: None,
            dropped: 0,
        })
    }

    /// Reject timestamps that move backwards or repeat within one source.
    /// Keep the newest frames when the bounded queue fills.
    pub fn push(&mut self, frame: TimedFrame) -> Result<(), FrameQueueError> {
        if self
            .last_target_ns
            .is_some_and(|target| frame.pts_ns <= target)
        {
            return Err(FrameQueueError::LateInput);
        }
        if self
            .pending
            .back()
            .or(self.last.as_ref())
            .is_some_and(|previous| frame.pts_ns <= previous.pts_ns)
        {
            return Err(FrameQueueError::NonMonotonicInput);
        }
        if self.pending.len() >= self.capacity {
            self.pending.pop_front();
            self.dropped += 1;
        }
        self.pending.push_back(frame);
        Ok(())
    }

    /// Select the newest frame with PTS <= output PTS, holding a previously
    /// selected frame if this source has not produced a newer one.
    ///
    /// Request times are monotonic, just like an actual output schedule.
    pub fn frame_at(&mut self, target_ns: u64) -> Result<Option<&RgbaFrame>, FrameQueueError> {
        self.peek_at(target_ns)?;
        self.last_target_ns = Some(target_ns);
        while self
            .pending
            .front()
            .is_some_and(|frame| frame.pts_ns <= target_ns)
        {
            self.last = self.pending.pop_front();
        }
        Ok(self.last.as_ref().map(|frame| &frame.rgba))
    }

    /// Inspect a candidate without consuming it or committing an output time.
    /// Failed multi-source renders must leave every queue unchanged.
    fn peek_at(&self, target_ns: u64) -> Result<Option<&RgbaFrame>, FrameQueueError> {
        if self
            .last_target_ns
            .is_some_and(|previous| target_ns < previous)
        {
            return Err(FrameQueueError::NonMonotonicOutput);
        }
        Ok(self
            .pending
            .iter()
            .rev()
            .find(|frame| frame.pts_ns <= target_ns)
            .or(self.last.as_ref())
            .map(|frame| &frame.rgba))
    }

    pub fn dropped_frames(&self) -> u64 {
        self.dropped
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    UnknownInput(InputId),
    UnavailableFrame(InputId),
    Queue(FrameQueueError),
    Composite(CompositeError),
}

impl From<FrameQueueError> for RenderError {
    fn from(error: FrameQueueError) -> Self {
        Self::Queue(error)
    }
}

impl From<CompositeError> for RenderError {
    fn from(error: CompositeError) -> Self {
        Self::Composite(error)
    }
}

/// Transition from Program to another source at explicit progress 0..=1.
/// This is a pixel-level reference, not a production transition transaction.
#[derive(Debug, Clone, Copy)]
pub struct ReferenceTake {
    pub destination: InputId,
    pub transition: Transition,
    pub progress: f32,
}

#[derive(Debug, Default)]
pub struct ReferenceOutput {
    sources: HashMap<InputId, FrameQueue>,
    last_rendered_pts_ns: Option<u64>,
}

impl ReferenceOutput {
    pub fn add_source(&mut self, input: InputId, capacity: usize) -> Result<(), FrameQueueError> {
        let queue = FrameQueue::new(capacity)?;
        self.sources.insert(input, queue);
        Ok(())
    }

    pub fn remove_source(&mut self, input: InputId) {
        self.sources.remove(&input);
    }

    pub fn push(&mut self, input: InputId, frame: TimedFrame) -> Result<(), RenderError> {
        self.sources
            .get_mut(&input)
            .ok_or(RenderError::UnknownInput(input))?
            .push(frame)?;
        Ok(())
    }

    fn sample(&self, input: InputId, pts_ns: u64) -> Result<&RgbaFrame, RenderError> {
        self.sources
            .get(&input)
            .ok_or(RenderError::UnknownInput(input))?
            .peek_at(pts_ns)?
            .ok_or(RenderError::UnavailableFrame(input))
    }

    /// Render one deterministic RGBA frame for a caller-supplied output PTS.
    ///
    /// Overlay array order is OV1 -> OV4, with OV4 composited last.
    /// Missing frames are explicit errors; never substitute another camera.
    /// Errors do not consume frames or advance any playout cursor. Successful
    /// render times remain monotonic even when Program or overlays change.
    pub fn render_at(
        &mut self,
        output_pts_ns: u64,
        program: InputId,
        take: Option<ReferenceTake>,
        overlay_inputs: [Option<InputId>; 4],
    ) -> Result<RgbaFrame, RenderError> {
        if self
            .last_rendered_pts_ns
            .is_some_and(|previous| output_pts_ns < previous)
        {
            return Err(FrameQueueError::NonMonotonicOutput.into());
        }
        // Reject invalid progress before dequeuing any source frames.
        // Invalid control input must not advance a source's playout cursor.
        if let Some(take) = take {
            if !take.progress.is_finite() || !(0.0..=1.0).contains(&take.progress) {
                return Err(RenderError::Composite(CompositeError::InvalidProgress));
            }
        }
        let program_frame = self.sample(program, output_pts_ns)?;
        let background = match take {
            Some(take) => {
                let destination = self.sample(take.destination, output_pts_ns)?;
                transition(program_frame, destination, take.transition, take.progress)?
            }
            None => program_frame.clone(),
        };

        let mut frames: [Option<&RgbaFrame>; 4] = [None, None, None, None];
        for (slot, input) in overlay_inputs.into_iter().enumerate() {
            if let Some(id) = input {
                frames[slot] = Some(self.sample(id, output_pts_ns)?);
            }
        }
        let composed = overlay_stack(&background, &frames)?;
        // Commit only after all source lookups and pixel composition succeed.
        // Repeated IDs are safe: frame_at permits the same timestamp twice.
        for id in std::iter::once(program)
            .chain(take.map(|take| take.destination))
            .chain(overlay_inputs.into_iter().flatten())
        {
            self.sources
                .get_mut(&id)
                .ok_or(RenderError::UnknownInput(id))?
                .frame_at(output_pts_ns)?;
        }
        self.last_rendered_pts_ns = Some(output_pts_ns);
        Ok(composed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(pts_ns: u64, rgba: [u8; 4]) -> TimedFrame {
        TimedFrame {
            pts_ns,
            rgba: RgbaFrame::solid(2, 1, rgba).unwrap(),
        }
    }

    #[test]
    fn bounded_queue_drops_oldest_and_preserves_latest() {
        let mut queue = FrameQueue::new(2).unwrap();
        queue.push(frame(0, [10, 0, 0, 255])).unwrap();
        queue.push(frame(20, [20, 0, 0, 255])).unwrap();
        queue.push(frame(40, [40, 0, 0, 255])).unwrap();
        assert_eq!(queue.dropped_frames(), 1);
        assert_eq!(queue.frame_at(20).unwrap().unwrap().pixels[0], 20);
        assert_eq!(queue.frame_at(40).unwrap().unwrap().pixels[0], 40);
        assert_eq!(queue.frame_at(60).unwrap().unwrap().pixels[0], 40);
    }

    #[test]
    fn no_future_frame_leaks_into_current_output() {
        let mut queue = FrameQueue::new(3).unwrap();
        queue.push(frame(100, [99, 0, 0, 255])).unwrap();
        assert!(queue.frame_at(99).unwrap().is_none());
        assert!(queue.frame_at(100).unwrap().is_some());
    }

    #[test]
    fn timestamps_must_be_monotonic_without_mutating_queue() {
        let mut queue = FrameQueue::new(3).unwrap();
        queue.push(frame(10, [1, 0, 0, 255])).unwrap();
        assert_eq!(
            queue.push(frame(10, [2, 0, 0, 255])),
            Err(FrameQueueError::NonMonotonicInput)
        );
        assert_eq!(
            queue.push(frame(9, [3, 0, 0, 255])),
            Err(FrameQueueError::NonMonotonicInput)
        );
        assert_eq!(queue.frame_at(10).unwrap().unwrap().pixels[0], 1);
        assert_eq!(queue.frame_at(9), Err(FrameQueueError::NonMonotonicOutput));
    }

    #[test]
    fn late_arriving_frame_does_not_rewind_playout() {
        let mut queue = FrameQueue::new(3).unwrap();
        queue.push(frame(0, [10, 0, 0, 255])).unwrap();
        assert_eq!(queue.frame_at(20).unwrap().unwrap().pixels[0], 10);
        assert_eq!(
            queue.push(frame(10, [99, 0, 0, 255])),
            Err(FrameQueueError::LateInput)
        );
        queue.push(frame(40, [40, 0, 0, 255])).unwrap();
        assert_eq!(queue.frame_at(40).unwrap().unwrap().pixels[0], 40);
    }

    #[test]
    fn reference_output_fades_and_applies_overlay_over_same_pts() {
        let mut output = ReferenceOutput::default();
        for id in [1, 2, 3] {
            output.add_source(id, 3).unwrap();
        }
        output.push(1, frame(0, [255, 0, 0, 255])).unwrap();
        output.push(2, frame(0, [0, 0, 255, 255])).unwrap();
        output.push(3, frame(0, [0, 255, 0, 255])).unwrap();
        let mid = output
            .render_at(
                0,
                1,
                Some(ReferenceTake {
                    destination: 2,
                    transition: Transition::Fade,
                    progress: 0.5,
                }),
                [None, None, None, None],
            )
            .unwrap();
        assert_eq!(&mid.pixels[..4], &[128, 0, 128, 255]);
        let with_overlay = output
            .render_at(20_000_000, 1, None, [Some(3), None, None, None])
            .unwrap();
        assert_eq!(&with_overlay.pixels[..4], &[0, 255, 0, 255]);
    }

    #[test]
    fn missing_device_never_silently_substitutes_another_source() {
        let mut output = ReferenceOutput::default();
        output.add_source(1, 3).unwrap();
        output.push(1, frame(0, [0, 0, 0, 255])).unwrap();
        assert_eq!(
            output.render_at(0, 99, None, [None; 4]),
            Err(RenderError::UnknownInput(99))
        );
        output.add_source(2, 3).unwrap();
        assert_eq!(
            output.render_at(0, 2, None, [None; 4]),
            Err(RenderError::UnavailableFrame(2))
        );
        output.remove_source(1);
        assert_eq!(
            output.render_at(0, 1, None, [None; 4]),
            Err(RenderError::UnknownInput(1))
        );
    }

    #[test]
    fn invalid_capacity_and_invalid_transition_are_rejected() {
        assert!(matches!(
            FrameQueue::new(0),
            Err(FrameQueueError::InvalidCapacity)
        ));
        let mut output = ReferenceOutput::default();
        output.add_source(1, 1).unwrap();
        output.push(1, frame(0, [0, 0, 0, 255])).unwrap();
        assert_eq!(
            output.render_at(
                0,
                1,
                Some(ReferenceTake {
                    destination: 1,
                    transition: Transition::Fade,
                    progress: f32::NAN,
                }),
                [None; 4],
            ),
            Err(RenderError::Composite(CompositeError::InvalidProgress))
        );
    }
    #[test]
    fn failed_render_does_not_consume_frames_or_advance_playout() {
        for failure in 0..3 {
            let mut output = ReferenceOutput::default();
            output.add_source(1, 3).unwrap();
            output.add_source(2, 3).unwrap();
            output.push(1, frame(0, [10, 0, 0, 255])).unwrap();
            output.push(1, frame(20, [20, 0, 0, 255])).unwrap();
            let overlays = match failure {
                0 => [Some(99), None, None, None],
                1 => [Some(2), None, None, None],
                _ => {
                    output
                        .push(
                            2,
                            TimedFrame {
                                pts_ns: 0,
                                rgba: RgbaFrame::solid(1, 1, [0, 0, 0, 255]).unwrap(),
                            },
                        )
                        .unwrap();
                    [Some(2), None, None, None]
                }
            };
            assert!(output.render_at(20, 1, None, overlays).is_err());
            let recovered = output.render_at(0, 1, None, [None; 4]).unwrap();
            assert_eq!(recovered.pixels[0], 10);
            output.push(1, frame(40, [40, 0, 0, 255])).unwrap();
            assert_eq!(
                output.render_at(20, 1, None, [None; 4]).unwrap().pixels[0],
                20
            );
        }
    }

    #[test]
    fn output_timeline_cannot_rewind_when_switching_sources() {
        let mut output = ReferenceOutput::default();
        for id in [1, 2] {
            output.add_source(id, 3).unwrap();
            output.push(id, frame(0, [0, 0, 0, 255])).unwrap();
        }
        output.render_at(20, 1, None, [None; 4]).unwrap();
        assert_eq!(
            output.render_at(10, 2, None, [None; 4]),
            Err(RenderError::Queue(FrameQueueError::NonMonotonicOutput))
        );
        // Re-rendering the same output PTS is allowed.
        assert!(output.render_at(20, 2, None, [None; 4]).is_ok());
    }
}
