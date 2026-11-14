//! Rational output deadlines and missed-deadline policy; no GUI or device clock.
use crate::{
    camera::FrameRate,
    media_timebase::{video_frame_at_ns, video_frame_pts_ns},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleError {
    InvalidRate,
    ClockRewound,
    Overflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputTick {
    pub index: u64,
    pub pts_ns: u64,
    /// Frames skipped before this tick; not encoder or capture drops.
    pub skipped_deadlines: u64,
    pub lateness_ns: u64,
}

pub struct OutputSchedule {
    rate: FrameRate,
    start_ns: u64,
    next_index: u64,
    last_now_ns: Option<u64>,
}

impl OutputSchedule {
    pub fn new(start_ns: u64, rate: FrameRate) -> Result<Self, ScheduleError> {
        if rate.numerator == 0
            || rate.denominator == 0
            || u64::from(rate.numerator) > 1_000_000_000 * u64::from(rate.denominator)
        {
            return Err(ScheduleError::InvalidRate);
        }
        Ok(Self {
            rate,
            start_ns,
            next_index: 0,
            last_now_ns: None,
        })
    }

    pub fn next_deadline_ns(&self) -> Result<u64, ScheduleError> {
        video_frame_pts_ns(self.next_index, self.rate)
            .and_then(|pts| self.start_ns.checked_add(pts))
            .ok_or(ScheduleError::Overflow)
    }

    /// Emit at most one tick per poll: choose the latest due deadline and skip
    /// older missed ones. Never burst-render backlog or shift the timeline.
    pub fn poll(&mut self, now_ns: u64) -> Result<Option<OutputTick>, ScheduleError> {
        if self.last_now_ns.is_some_and(|last| now_ns < last) {
            return Err(ScheduleError::ClockRewound);
        }
        if now_ns < self.next_deadline_ns()? {
            self.last_now_ns = Some(now_ns);
            return Ok(None);
        }
        let index =
            video_frame_at_ns(now_ns - self.start_ns, self.rate).ok_or(ScheduleError::Overflow)?;
        let pts_ns = video_frame_pts_ns(index, self.rate)
            .and_then(|pts| self.start_ns.checked_add(pts))
            .ok_or(ScheduleError::Overflow)?;
        let next = index.checked_add(1).ok_or(ScheduleError::Overflow)?;
        // Ensure the following deadline is representable before committing.
        video_frame_pts_ns(next, self.rate)
            .and_then(|pts| self.start_ns.checked_add(pts))
            .ok_or(ScheduleError::Overflow)?;
        let tick = OutputTick {
            index,
            pts_ns,
            skipped_deadlines: index - self.next_index,
            lateness_ns: now_ns - pts_ns,
        };
        self.next_index = next;
        self.last_now_ns = Some(now_ns);
        Ok(Some(tick))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_50fps_deadlines_and_no_early_or_duplicate_ticks() {
        let mut schedule = OutputSchedule::new(100, FrameRate::PAL_50).unwrap();
        assert_eq!(schedule.poll(99), Ok(None));
        assert_eq!(schedule.poll(100).unwrap().unwrap().index, 0);
        assert_eq!(schedule.poll(100), Ok(None));
        assert_eq!(schedule.poll(20_000_099), Ok(None));
        let tick = schedule.poll(20_000_100).unwrap().unwrap();
        assert_eq!(
            (tick.index, tick.pts_ns, tick.lateness_ns),
            (1, 20_000_100, 0)
        );
    }
    #[test]
    fn fractional_deadlines_have_no_cumulative_rounding_drift() {
        let rate = FrameRate::new(60_000, 1001).unwrap();
        let mut schedule = OutputSchedule::new(0, rate).unwrap();
        for index in 0..10_000 {
            let pts = video_frame_pts_ns(index, rate).unwrap();
            let tick = schedule.poll(pts).unwrap().unwrap();
            assert_eq!(
                (tick.index, tick.pts_ns, tick.skipped_deadlines),
                (index, pts, 0)
            );
        }
    }
    #[test]
    fn slow_worker_skips_backlog_and_preserves_absolute_phase() {
        let mut schedule = OutputSchedule::new(0, FrameRate::PAL_50).unwrap();
        schedule.poll(0).unwrap();
        let tick = schedule.poll(105_000_000).unwrap().unwrap();
        assert_eq!(
            (tick.index, tick.skipped_deadlines, tick.lateness_ns),
            (5, 4, 5_000_000)
        );
        assert_eq!(schedule.next_deadline_ns(), Ok(120_000_000));
        assert_eq!(schedule.poll(105_000_000), Ok(None));
        assert_eq!(schedule.poll(104_000_000), Err(ScheduleError::ClockRewound));
        assert_eq!(schedule.poll(120_000_000).unwrap().unwrap().index, 6);
    }
    #[test]
    fn invalid_rates_and_overflow_are_errors() {
        assert!(matches!(
            OutputSchedule::new(
                0,
                FrameRate {
                    numerator: 0,
                    denominator: 1
                }
            ),
            Err(ScheduleError::InvalidRate)
        ));
        assert!(matches!(
            OutputSchedule::new(
                0,
                FrameRate {
                    numerator: u32::MAX,
                    denominator: 1
                }
            ),
            Err(ScheduleError::InvalidRate)
        ));
        let mut schedule = OutputSchedule::new(u64::MAX - 1, FrameRate::PAL_50).unwrap();
        assert_eq!(schedule.poll(u64::MAX - 1), Err(ScheduleError::Overflow));
        assert_eq!(schedule.next_deadline_ns(), Ok(u64::MAX - 1));
    }
}
