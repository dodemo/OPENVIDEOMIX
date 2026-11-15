//! Deterministic, platform-independent RGBA reference compositor.
//!
//! This reference implementation establishes pixel semantics for CUT, FADE,
//! WIPE and alpha overlays. It is NOT a real-time or GPU-accelerated media
//! engine. A future wgpu implementation must produce equivalent test frames.

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
}
