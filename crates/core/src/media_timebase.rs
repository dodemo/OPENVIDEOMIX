//! Exact rational media timelines, independent of GUI repaint cadence.
//!
//! This module converts video frame numbers and audio sample numbers to
//! presentation timestamps in nanoseconds. It is not a realtime clock,
//! device synchronizer, or measured output frame-rate indicator.

use crate::camera::FrameRate;

/// Integer nanoseconds per second, shared by audio and video timebases.
pub const NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;

/// Floor-rounded timestamp of a video frame, starting at timestamp zero.
///
/// Use u128 intermediates to preserve the 60000/1001 rate exactly and
/// avoid accumulating a rounded frame period over long productions.
/// Return None for invalid rates or timestamps outside the u64 range.
pub fn video_frame_pts_ns(frame_index: u64, rate: FrameRate) -> Option<u64> {
    if rate.numerator == 0 || rate.denominator == 0 {
        return None;
    }
    let numerator = u128::from(frame_index)
        .checked_mul(u128::from(rate.denominator))?
        .checked_mul(u128::from(NANOSECONDS_PER_SECOND))?;
    u64::try_from(numerator / u128::from(rate.numerator)).ok()
}

/// Return the frame whose floor-rounded PTS is at or before this time.
///
/// The inverse must use the exact same floor-rounding rule as
/// `video_frame_pts_ns`. A naive floor(t * fps) returns frame 0 at the
/// nanosecond where frame 1 starts for fractional rates such as 60000/1001.
/// This is a timeline lookup, not a measured capture/output FPS.
pub fn video_frame_at_ns(timestamp_ns: u64, rate: FrameRate) -> Option<u64> {
    if rate.numerator == 0 || rate.denominator == 0 {
        return None;
    }
    let numerator = (u128::from(timestamp_ns) + 1)
        .checked_mul(u128::from(rate.numerator))?
        .checked_sub(1)?;
    let denominator = u128::from(NANOSECONDS_PER_SECOND)
        .checked_mul(u128::from(rate.denominator))?;
    u64::try_from(numerator / denominator).ok()
}

/// Floor-rounded presentation timestamp of an audio sample at its actual
/// samples-per-second rate. 48 kHz is the expected production configuration.
pub fn audio_sample_pts_ns(sample_index: u64, sample_rate_hz: u32) -> Option<u64> {
    if sample_rate_hz == 0 {
        return None;
    }
    let scaled = u128::from(sample_index)
        .checked_mul(u128::from(NANOSECONDS_PER_SECOND))?;
    u64::try_from(scaled / u128::from(sample_rate_hz)).ok()
}

/// Duration in ns from an integral number of audio samples. This is also
/// floor-rounded; it does not infer sample capture times or correct drift.
pub fn audio_samples_duration_ns(samples: u64, sample_rate_hz: u32) -> Option<u64> {
    audio_sample_pts_ns(samples, sample_rate_hz)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pal_50_produces_exact_20_millisecond_spacing() {
        for index in [0_u64, 1, 2, 49, 50, 50_000] {
            assert_eq!(
                video_frame_pts_ns(index, FrameRate::PAL_50),
                Some(index * 20_000_000)
            );
        }
        assert_eq!(
            video_frame_at_ns(999_999_999, FrameRate::PAL_50),
            Some(49)
        );
        assert_eq!(
            video_frame_at_ns(1_000_000_000, FrameRate::PAL_50),
            Some(50)
        );
    }

    #[test]
    fn fractional_5994_has_no_cumulative_fps_rounding_error() {
        let rate = FrameRate::new(60_000, 1_001).unwrap();
        assert_eq!(video_frame_pts_ns(1, rate), Some(16_683_333));
        // 60,000 frames at 60000/1001 fps is exactly 1001 seconds.
        assert_eq!(
            video_frame_pts_ns(60_000, rate),
            Some(1_001_000_000_000)
        );
        assert_eq!(
            video_frame_at_ns(1_001_000_000_000, rate),
            Some(60_000)
        );
    }

    #[test]
    fn fractional_rate_inverse_matches_floor_rounded_frame_start() {
        let rate = FrameRate::new(60_000, 1_001).unwrap();
        for frame in 1..=10_000 {
            let start = video_frame_pts_ns(frame, rate).unwrap();
            assert_eq!(
                video_frame_at_ns(start, rate),
                Some(frame),
                "frame {frame} must be selected at its exact integer PTS"
            );
            assert_eq!(
                video_frame_at_ns(start - 1, rate),
                Some(frame - 1),
                "the previous nanosecond belongs to the prior frame"
            );
        }
    }

    #[test]
    fn half_second_of_audio_and_video_share_one_timestamp() {
        let video = video_frame_pts_ns(25, FrameRate::PAL_50);
        let audio = audio_sample_pts_ns(24_000, 48_000);
        assert_eq!(video, Some(500_000_000));
        assert_eq!(audio, video);
        assert_eq!(
            audio_samples_duration_ns(48_000, 48_000),
            Some(NANOSECONDS_PER_SECOND)
        );
    }

    #[test]
    fn fractional_audio_sample_timestamps_use_floor_rounding() {
        assert_eq!(audio_sample_pts_ns(1, 48_000), Some(20_833));
        assert_eq!(audio_sample_pts_ns(3, 48_000), Some(62_500));
        assert_eq!(audio_sample_pts_ns(0, 48_000), Some(0));
    }

    #[test]
    fn invalid_rate_is_rejected_without_division_by_zero() {
        let invalid = FrameRate {
            numerator: 0,
            denominator: 1,
        };
        let invalid_denominator = FrameRate {
            numerator: 50,
            denominator: 0,
        };
        assert_eq!(video_frame_pts_ns(1, invalid), None);
        assert_eq!(video_frame_at_ns(1, invalid_denominator), None);
        assert_eq!(audio_sample_pts_ns(1, 0), None);
    }

    #[test]
    fn out_of_range_timestamps_are_rejected_instead_of_wrapping() {
        assert_eq!(video_frame_pts_ns(u64::MAX, FrameRate::PAL_50), None);
        assert_eq!(audio_sample_pts_ns(u64::MAX, 48_000), None);
        assert_eq!(
            video_frame_at_ns(u64::MAX, FrameRate::new(u32::MAX, 1).unwrap()),
            None
        );
    }

    #[test]
    fn frame_timestamps_are_monotonic_at_common_broadcast_rates() {
        let rates = [
            FrameRate::new(25, 1).unwrap(),
            FrameRate::PAL_50,
            FrameRate::new(60, 1).unwrap(),
            FrameRate::new(60_000, 1_001).unwrap(),
        ];
        for rate in rates {
            let mut previous = 0;
            for index in 1..1_000 {
                let next = video_frame_pts_ns(index, rate).unwrap();
                assert!(next > previous);
                previous = next;
            }
        }
    }
}
