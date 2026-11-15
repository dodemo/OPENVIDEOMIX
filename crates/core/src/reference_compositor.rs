//! Deterministic, platform-independent RGBA reference compositor.
//!
//! This reference implementation establishes pixel semantics for CUT, FADE,
//! WIPE and alpha overlays. It is NOT a real-time or GPU-accelerated media
//! engine. A future wgpu implementation must produce equivalent test frames.

use crate::InputId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositeError {
    InvalidDimensions,
    BufferSize,
    DimensionMismatch,
    InvalidProgress,
}

/// Unpremultiplied RGBA8 pixels, row-major, tightly packed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl RgbaFrame {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, CompositeError> {
        if width == 0 || height == 0 {
            return Err(CompositeError::InvalidDimensions);
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or(CompositeError::InvalidDimensions)?;
        if pixels.len() != expected {
            return Err(CompositeError::BufferSize);
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn solid(width: u32, height: u32, color: [u8; 4]) -> Result<Self, CompositeError> {
        let len = (width as usize)
            .checked_mul(height as usize)
            .ok_or(CompositeError::InvalidDimensions)?;
        if width == 0 || height == 0 {
            return Err(CompositeError::InvalidDimensions);
        }
        let capacity = len
            .checked_mul(4)
            .ok_or(CompositeError::InvalidDimensions)?;
        let mut pixels = Vec::with_capacity(capacity);
        for _ in 0..len {
            pixels.extend_from_slice(&color);
        }
        Self::new(width, height, pixels)
    }

    fn same_dimensions(&self, other: &Self) -> Result<(), CompositeError> {
        if self.width == other.width && self.height == other.height {
            Ok(())
        } else {
            Err(CompositeError::DimensionMismatch)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    Cut,
    Fade,
    Mix,
    WipeLeftToRight,
    WipeRightToLeft,
    WipeTopToBottom,
    WipeBottomToTop,
}

/// Compose between frames at progress 0..=1. CUT is an instantaneous take:
/// it selects the destination for any nonzero progress.
pub fn transition(
    from: &RgbaFrame,
    to: &RgbaFrame,
    kind: Transition,
    progress: f32,
) -> Result<RgbaFrame, CompositeError> {
    from.same_dimensions(to)?;
    if !progress.is_finite() || !(0.0..=1.0).contains(&progress) {
        return Err(CompositeError::InvalidProgress);
    }
    if progress == 0.0 {
        return Ok(from.clone());
    }
    if kind == Transition::Cut || progress == 1.0 {
        return Ok(to.clone());
    }
    let mut result = from.clone();
    match kind {
        Transition::Cut => unreachable!(),
        Transition::Fade | Transition::Mix => {
            for ((a, b), output) in from
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .zip(to.pixels.as_chunks::<4>().0.iter())
                .zip(result.pixels.as_chunks_mut::<4>().0.iter_mut())
            {
                let old_alpha = f32::from(a[3]) / 255.0;
                let new_alpha = f32::from(b[3]) / 255.0;
                let alpha = old_alpha * (1.0 - progress) + new_alpha * progress;
                for channel in 0..3 {
                    let premultiplied = f32::from(a[channel]) * old_alpha * (1.0 - progress)
                        + f32::from(b[channel]) * new_alpha * progress;
                    output[channel] = if alpha > 0.0 {
                        (premultiplied / alpha).round().clamp(0.0, 255.0) as u8
                    } else {
                        0
                    };
                }
                output[3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
        Transition::WipeLeftToRight => {
            let reveal_columns = (from.width as f64 * f64::from(progress)).floor() as usize;
            let width = from.width as usize;
            for (pixel_index, output) in result.pixels.as_chunks_mut::<4>().0.iter_mut().enumerate()
            {
                if pixel_index % width < reveal_columns {
                    output.copy_from_slice(&to.pixels[pixel_index * 4..pixel_index * 4 + 4]);
                }
            }
        }
        Transition::WipeRightToLeft => {
            let reveal_columns = (from.width as f64 * f64::from(progress)).floor() as usize;
            let width = from.width as usize;
            let start_col = width.saturating_sub(reveal_columns);
            for (pixel_index, output) in result.pixels.as_chunks_mut::<4>().0.iter_mut().enumerate()
            {
                if pixel_index % width >= start_col {
                    output.copy_from_slice(&to.pixels[pixel_index * 4..pixel_index * 4 + 4]);
                }
            }
        }
        Transition::WipeTopToBottom => {
            let reveal_rows = (from.height as f64 * f64::from(progress)).floor() as usize;
            let width = from.width as usize;
            for (pixel_index, output) in result.pixels.as_chunks_mut::<4>().0.iter_mut().enumerate()
            {
                let row = pixel_index / width;
                if row < reveal_rows {
                    output.copy_from_slice(&to.pixels[pixel_index * 4..pixel_index * 4 + 4]);
                }
            }
        }
        Transition::WipeBottomToTop => {
            let reveal_rows = (from.height as f64 * f64::from(progress)).floor() as usize;
            let height = from.height as usize;
            let width = from.width as usize;
            let start_row = height.saturating_sub(reveal_rows);
            for (pixel_index, output) in result.pixels.as_chunks_mut::<4>().0.iter_mut().enumerate()
            {
                let row = pixel_index / width;
                if row >= start_row {
                    output.copy_from_slice(&to.pixels[pixel_index * 4..pixel_index * 4 + 4]);
                }
            }
        }
    }
    Ok(result)
}

/// Alpha-composite an overlay onto the base at an arbitrary (dest_x, dest_y) coordinate.
/// The overlay can be smaller than the base frame (e.g. lower thirds, channel bugs, PIP).
/// Out-of-bounds regions are clipped gracefully.
pub fn overlay_at(
    base: &RgbaFrame,
    layer: &RgbaFrame,
    dest_x: i32,
    dest_y: i32,
) -> Result<RgbaFrame, CompositeError> {
    if base.width == 0 || base.height == 0 || layer.width == 0 || layer.height == 0 {
        return Err(CompositeError::InvalidDimensions);
    }
    let mut result = base.clone();
    let base_w = base.width as i32;
    let base_h = base.height as i32;
    let layer_w = layer.width as i32;
    let layer_h = layer.height as i32;

    for ly in 0..layer_h {
        let by = dest_y + ly;
        if by < 0 || by >= base_h {
            continue;
        }
        for lx in 0..layer_w {
            let bx = dest_x + lx;
            if bx < 0 || bx >= base_w {
                continue;
            }

            let base_idx = ((by as usize) * (base.width as usize) + (bx as usize)) * 4;
            let layer_idx = ((ly as usize) * (layer.width as usize) + (lx as usize)) * 4;

            let background = &base.pixels[base_idx..base_idx + 4];
            let foreground = &layer.pixels[layer_idx..layer_idx + 4];

            let a = f32::from(background[3]) / 255.0;
            let b = f32::from(foreground[3]) / 255.0;
            let out_alpha = b + a * (1.0 - b);

            let out = &mut result.pixels[base_idx..base_idx + 4];
            for channel in 0..3 {
                let premultiplied = f32::from(foreground[channel]) * b
                    + f32::from(background[channel]) * a * (1.0 - b);
                out[channel] = if out_alpha > 0.0 {
                    (premultiplied / out_alpha).round().clamp(0.0, 255.0) as u8
                } else {
                    0
                };
            }
            out[3] = (out_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    Ok(result)
}

/// Alpha-composite an overlay onto the base in straight-alpha RGBA.
/// Both frames share the same dimensions in this full-screen overlay function.
pub fn overlay(base: &RgbaFrame, layer: &RgbaFrame) -> Result<RgbaFrame, CompositeError> {
    base.same_dimensions(layer)?;
    overlay_at(base, layer, 0, 0)
}

/// Apply up to four independently provided overlays, bottom to top.
/// The provided order determines layering and is not changed.
pub fn overlay_stack(
    base: &RgbaFrame,
    layers: &[Option<&RgbaFrame>; 4],
) -> Result<RgbaFrame, CompositeError> {
    let mut frame = base.clone();
    for layer in layers.iter().flatten() {
        frame = overlay(&frame, layer)?;
    }
    Ok(frame)
}

/// A Stinger transition sequence consisting of alpha-composited animation frames
/// and an explicit cut frame index where background switching takes place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StingerSequence {
    pub frames: Vec<RgbaFrame>,
    pub cut_frame_index: usize,
}

impl StingerSequence {
    pub fn new(frames: Vec<RgbaFrame>, cut_frame_index: usize) -> Result<Self, CompositeError> {
        if frames.is_empty() {
            return Err(CompositeError::InvalidDimensions);
        }
        if cut_frame_index >= frames.len() {
            return Err(CompositeError::InvalidProgress);
        }
        let w = frames[0].width;
        let h = frames[0].height;
        for frame in &frames {
            if frame.width != w || frame.height != h {
                return Err(CompositeError::DimensionMismatch);
            }
        }
        Ok(Self {
            frames,
            cut_frame_index,
        })
    }

    #[inline]
    pub fn total_frames(&self) -> usize {
        self.frames.len()
    }

    /// Determines if the destination frame (to) should be active at the given progress.
    #[inline]
    pub fn is_destination_active(&self, progress: f32) -> Result<bool, CompositeError> {
        if !progress.is_finite() || !(0.0..=1.0).contains(&progress) {
            return Err(CompositeError::InvalidProgress);
        }
        let frame_idx = ((self.frames.len() - 1) as f32 * progress)
            .round()
            .clamp(0.0, (self.frames.len() - 1) as f32) as usize;
        Ok(frame_idx >= self.cut_frame_index)
    }

    /// Compose the output at a given transition progress (0.0..=1.0).
    /// Before the cut frame index: background is `from`.
    /// At and after the cut frame index: background is `to`.
    /// The stinger animation frame at `progress` is alpha-composited on top of the active background.
    pub fn compose(
        &self,
        from: &RgbaFrame,
        to: &RgbaFrame,
        progress: f32,
    ) -> Result<RgbaFrame, CompositeError> {
        from.same_dimensions(to)?;
        if !progress.is_finite() || !(0.0..=1.0).contains(&progress) {
            return Err(CompositeError::InvalidProgress);
        }
        let frame_idx = ((self.frames.len() - 1) as f32 * progress)
            .round()
            .clamp(0.0, (self.frames.len() - 1) as f32) as usize;
        let background = if frame_idx < self.cut_frame_index {
            from
        } else {
            to
        };
        background.same_dimensions(&self.frames[frame_idx])?;
        overlay(background, &self.frames[frame_idx])
    }
}

/// Active transition transaction between sources with monotonic timing and progress tracking.
#[derive(Debug, Clone, PartialEq)]
pub struct TransitionTransaction {
    pub from_source: Option<InputId>,
    pub to_source: InputId,
    pub start_timestamp_ns: u64,
    pub duration_ns: u64,
    pub progress: f32,
    pub is_completed: bool,
}

impl TransitionTransaction {
    pub fn new(
        from_source: Option<InputId>,
        to_source: InputId,
        is_cut: bool,
        start_timestamp_ns: u64,
        duration_ms: u32,
    ) -> Self {
        let duration_ns = (duration_ms as u64) * 1_000_000;
        let immediate = is_cut || duration_ms == 0;
        Self {
            from_source,
            to_source,
            start_timestamp_ns,
            duration_ns,
            progress: if immediate { 1.0 } else { 0.0 },
            is_completed: immediate,
        }
    }

    /// Advance transition given the current monotonic timestamp in nanoseconds.
    pub fn update(&mut self, current_timestamp_ns: u64) -> f32 {
        if self.is_completed {
            return 1.0;
        }
        if current_timestamp_ns <= self.start_timestamp_ns {
            self.progress = 0.0;
            return 0.0;
        }
        let elapsed_ns = current_timestamp_ns.saturating_sub(self.start_timestamp_ns);
        if self.duration_ns == 0 || elapsed_ns >= self.duration_ns {
            self.progress = 1.0;
            self.is_completed = true;
        } else {
            self.progress = (elapsed_ns as f64 / self.duration_ns as f64).clamp(0.0, 1.0) as f32;
        }
        self.progress
    }

    /// Interrupt/cancel current transition immediately (takes to_source directly).
    pub fn cancel_to_cut(&mut self) {
        self.progress = 1.0;
        self.is_completed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(frame: &RgbaFrame, x: usize) -> [u8; 4] {
        frame.pixels[x * 4..x * 4 + 4].try_into().unwrap()
    }

    #[test]
    fn cut_switches_at_nonzero_progress() {
        let a = RgbaFrame::solid(2, 1, [255, 0, 0, 255]).unwrap();
        let b = RgbaFrame::solid(2, 1, [0, 0, 255, 255]).unwrap();
        assert_eq!(transition(&a, &b, Transition::Cut, 0.0).unwrap(), a);
        assert_eq!(transition(&a, &b, Transition::Cut, 0.01).unwrap(), b);
    }

    #[test]
    fn fade_renders_midpoint_not_a_cut() {
        let a = RgbaFrame::solid(1, 1, [255, 0, 0, 255]).unwrap();
        let b = RgbaFrame::solid(1, 1, [0, 0, 255, 255]).unwrap();
        let middle = transition(&a, &b, Transition::Fade, 0.5).unwrap();
        assert_eq!(pixel(&middle, 0), [128, 0, 128, 255]);
    }

    #[test]
    fn wipe_reveals_destination_columns_without_swapping_all_pixels() {
        let a = RgbaFrame::solid(4, 1, [255, 0, 0, 255]).unwrap();
        let b = RgbaFrame::solid(4, 1, [0, 0, 255, 255]).unwrap();
        let middle = transition(&a, &b, Transition::WipeLeftToRight, 0.5).unwrap();
        for x in 0..4 {
            assert_eq!(
                pixel(&middle, x),
                if x < 2 {
                    [0, 0, 255, 255]
                } else {
                    [255, 0, 0, 255]
                }
            );
        }
    }

    #[test]
    fn alpha_overlay_preserves_background_for_transparent_pixels() {
        let base = RgbaFrame::solid(1, 1, [40, 80, 120, 255]).unwrap();
        let transparent = RgbaFrame::solid(1, 1, [255, 0, 0, 0]).unwrap();
        assert_eq!(overlay(&base, &transparent).unwrap(), base);
        let opaque = RgbaFrame::solid(1, 1, [200, 0, 0, 255]).unwrap();
        assert_eq!(overlay(&base, &opaque).unwrap(), opaque);
    }

    #[test]
    fn four_overlays_follow_declared_z_order() {
        let base = RgbaFrame::solid(1, 1, [0, 0, 0, 255]).unwrap();
        let first = RgbaFrame::solid(1, 1, [255, 0, 0, 255]).unwrap();
        let fourth = RgbaFrame::solid(1, 1, [0, 255, 0, 255]).unwrap();
        let result = overlay_stack(&base, &[Some(&first), None, None, Some(&fourth)]).unwrap();
        assert_eq!(result.pixels, fourth.pixels);
    }

    #[test]
    fn rejects_bad_dimensions_or_invalid_transition_progress() {
        assert_eq!(
            RgbaFrame::new(1, 1, vec![0; 3]),
            Err(CompositeError::BufferSize)
        );
        let a = RgbaFrame::solid(1, 1, [0, 0, 0, 255]).unwrap();
        let b = RgbaFrame::solid(2, 1, [0, 0, 0, 255]).unwrap();
        assert_eq!(
            transition(&a, &b, Transition::Fade, 0.5),
            Err(CompositeError::DimensionMismatch)
        );
        assert_eq!(
            transition(&a, &a, Transition::Fade, f32::NAN),
            Err(CompositeError::InvalidProgress)
        );
    }

    #[test]
    fn mix_transition_matches_fade_pixel_blend() {
        let a = RgbaFrame::solid(1, 1, [255, 0, 0, 255]).unwrap();
        let b = RgbaFrame::solid(1, 1, [0, 0, 255, 255]).unwrap();
        let fade_result = transition(&a, &b, Transition::Fade, 0.5).unwrap();
        let mix_result = transition(&a, &b, Transition::Mix, 0.5).unwrap();
        assert_eq!(fade_result, mix_result);
        assert_eq!(pixel(&mix_result, 0), [128, 0, 128, 255]);
    }

    #[test]
    fn wipe_directions_geometry_tests() {
        let red = RgbaFrame::solid(4, 4, [255, 0, 0, 255]).unwrap();
        let blue = RgbaFrame::solid(4, 4, [0, 0, 255, 255]).unwrap();

        // WipeRightToLeft at 50%: columns 2 and 3 should be blue, 0 and 1 red
        let wipe_rl = transition(&red, &blue, Transition::WipeRightToLeft, 0.5).unwrap();
        for y in 0..4 {
            for x in 0..4 {
                let p = pixel(&wipe_rl, y * 4 + x);
                assert_eq!(
                    p,
                    if x >= 2 {
                        [0, 0, 255, 255]
                    } else {
                        [255, 0, 0, 255]
                    }
                );
            }
        }

        // WipeTopToBottom at 50%: rows 0 and 1 should be blue, 2 and 3 red
        let wipe_tb = transition(&red, &blue, Transition::WipeTopToBottom, 0.5).unwrap();
        for y in 0..4 {
            for x in 0..4 {
                let p = pixel(&wipe_tb, y * 4 + x);
                assert_eq!(
                    p,
                    if y < 2 {
                        [0, 0, 255, 255]
                    } else {
                        [255, 0, 0, 255]
                    }
                );
            }
        }

        // WipeBottomToTop at 50%: rows 2 and 3 should be blue, 0 and 1 red
        let wipe_bt = transition(&red, &blue, Transition::WipeBottomToTop, 0.5).unwrap();
        for y in 0..4 {
            for x in 0..4 {
                let p = pixel(&wipe_bt, y * 4 + x);
                assert_eq!(
                    p,
                    if y >= 2 {
                        [0, 0, 255, 255]
                    } else {
                        [255, 0, 0, 255]
                    }
                );
            }
        }
    }

    #[test]
    fn overlay_at_positions_smaller_layer_with_clipping() {
        let base = RgbaFrame::solid(4, 4, [0, 0, 0, 255]).unwrap();
        // A 2x2 green patch placed at (1, 1)
        let patch = RgbaFrame::solid(2, 2, [0, 255, 0, 255]).unwrap();
        let result = overlay_at(&base, &patch, 1, 1).unwrap();

        for y in 0..4 {
            for x in 0..4 {
                let p = pixel(&result, y * 4 + x);
                if (1..=2).contains(&x) && (1..=2).contains(&y) {
                    assert_eq!(p, [0, 255, 0, 255]);
                } else {
                    assert_eq!(p, [0, 0, 0, 255]);
                }
            }
        }
    }

    #[test]
    fn stinger_validates_dimensions_and_cut_frame() {
        assert_eq!(
            StingerSequence::new(vec![], 0),
            Err(CompositeError::InvalidDimensions)
        );
        let f1 = RgbaFrame::solid(2, 2, [255, 255, 255, 0]).unwrap();
        let f2_mismatch = RgbaFrame::solid(3, 2, [255, 255, 255, 0]).unwrap();
        assert_eq!(
            StingerSequence::new(vec![f1.clone(), f2_mismatch], 0),
            Err(CompositeError::DimensionMismatch)
        );
        assert_eq!(
            StingerSequence::new(vec![f1.clone()], 1),
            Err(CompositeError::InvalidProgress)
        );
        assert!(StingerSequence::new(vec![f1], 0).is_ok());
    }

    #[test]
    fn stinger_switches_background_at_cut_frame_and_blends_animation() {
        let from = RgbaFrame::solid(2, 2, [255, 0, 0, 255]).unwrap(); // Red
        let to = RgbaFrame::solid(2, 2, [0, 0, 255, 255]).unwrap(); // Blue

        // 3-frame stinger animation:
        // Frame 0: Semi-transparent white (alpha 128)
        // Frame 1: Fully opaque black (covers screen completely, cut point)
        // Frame 2: Semi-transparent white (alpha 128)
        let s0 = RgbaFrame::solid(2, 2, [255, 255, 255, 128]).unwrap();
        let s1 = RgbaFrame::solid(2, 2, [0, 0, 0, 255]).unwrap();
        let s2 = RgbaFrame::solid(2, 2, [255, 255, 255, 128]).unwrap();

        let stinger = StingerSequence::new(vec![s0, s1, s2], 1).unwrap();
        assert_eq!(stinger.total_frames(), 3);

        // At progress 0.0: frame 0, before cut point -> background is 'from' (Red)
        assert!(!stinger.is_destination_active(0.0).unwrap());
        let res0 = stinger.compose(&from, &to, 0.0).unwrap();
        // Red blended with semi-transparent white: [255, 128, 128, 255]
        let p0 = pixel(&res0, 0);
        assert_eq!(p0[0], 255);
        assert_eq!(p0[1], 128);
        assert_eq!(p0[2], 128);

        // At progress 0.5: frame 1, at cut point -> background is 'to' (Blue), but stinger is opaque black
        assert!(stinger.is_destination_active(0.5).unwrap());
        let res1 = stinger.compose(&from, &to, 0.5).unwrap();
        assert_eq!(pixel(&res1, 0), [0, 0, 0, 255]);

        // At progress 1.0: frame 2, after cut point -> background is 'to' (Blue)
        assert!(stinger.is_destination_active(1.0).unwrap());
        let res2 = stinger.compose(&from, &to, 1.0).unwrap();
        // Blue blended with semi-transparent white: [128, 128, 255, 255]
        let p2 = pixel(&res2, 0);
        assert_eq!(p2[0], 128);
        assert_eq!(p2[1], 128);
        assert_eq!(p2[2], 255);
    }

    #[test]
    fn transition_transaction_tracks_monotonic_progress_and_completion() {
        let mut tx = TransitionTransaction::new(Some(1), 2, false, 1_000_000_000, 500);
        assert_eq!(tx.progress, 0.0);
        assert!(!tx.is_completed);

        // Before start timestamp
        assert_eq!(tx.update(500_000_000), 0.0);
        assert!(!tx.is_completed);

        // Exactly halfway (250 ms = 250_000_000 ns elapsed)
        let mid = tx.update(1_250_000_000);
        assert!((mid - 0.5).abs() < 1e-4);
        assert!(!tx.is_completed);

        // Finished (500 ms elapsed)
        let done = tx.update(1_500_000_000);
        assert_eq!(done, 1.0);
        assert!(tx.is_completed);

        // An immediate cut transaction starts completed
        let cut_tx = TransitionTransaction::new(Some(1), 2, true, 1_000_000_000, 500);
        assert_eq!(cut_tx.progress, 1.0);
        assert!(cut_tx.is_completed);

        // Cancelling takes immediately to cut
        let mut cancelled = TransitionTransaction::new(Some(1), 2, false, 1_000_000_000, 500);
        cancelled.cancel_to_cut();
        assert_eq!(cancelled.progress, 1.0);
        assert!(cancelled.is_completed);
    }
}
