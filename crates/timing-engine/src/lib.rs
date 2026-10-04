use stagecue_project_model::{
    EventTiming, MeterChange, MusicalPosition, SongSection, TempoChange, TempoTransition, PPQ,
};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum TimingError {
    #[error("tempo map must start at or before bar 1 and contain positive finite BPM values")]
    InvalidTempoMap,
    #[error("meter map must start at or before bar 1, change only on bar boundaries, and use valid meters")]
    InvalidMeterMap,
    #[error("position has an invalid beat or tick for its meter")]
    InvalidPosition,
    #[error("sample rate must be positive")]
    InvalidSampleRate,
    #[error("time must be finite")]
    InvalidTime,
}

#[derive(Debug, Clone)]
struct TempoPoint {
    tick: i64,
    bpm: f64,
    transition: TempoTransition,
}

#[derive(Debug, Clone)]
pub struct Timeline {
    meters: Vec<MeterChange>,
    tempos: Vec<TempoPoint>,
}

impl Timeline {
    pub fn new(
        mut meters: Vec<MeterChange>,
        tempos: Vec<TempoChange>,
    ) -> Result<Self, TimingError> {
        meters.sort_by_key(|m| (m.position.bar, m.position.beat, m.position.tick));
        if meters.is_empty()
            || meters[0].position.bar > 1
            || meters.iter().any(|m| {
                m.position.beat != 1
                    || m.position.tick != 0
                    || m.numerator == 0
                    || !matches!(m.denominator, 1 | 2 | 4 | 8 | 16 | 32)
            })
        {
            return Err(TimingError::InvalidMeterMap);
        }
        let shell = Self {
            meters,
            tempos: vec![],
        };
        let mut points = Vec::with_capacity(tempos.len());
        for tempo in tempos {
            if !tempo.bpm.is_finite() || tempo.bpm <= 0.0 {
                return Err(TimingError::InvalidTempoMap);
            }
            points.push(TempoPoint {
                tick: shell.position_to_ticks(tempo.position)?,
                bpm: tempo.bpm,
                transition: tempo.transition,
            });
        }
        points.sort_by_key(|p| p.tick);
        if points.is_empty()
            || points[0].tick > 0
            || points.windows(2).any(|w| w[0].tick == w[1].tick)
        {
            return Err(TimingError::InvalidTempoMap);
        }
        Ok(Self {
            meters: shell.meters,
            tempos: points,
        })
    }

    pub fn position_to_ticks(&self, position: MusicalPosition) -> Result<i64, TimingError> {
        if position.beat == 0 || position.tick >= PPQ {
            return Err(TimingError::InvalidPosition);
        }
        let meter = self.meter_at_bar(position.bar);
        let beat_ticks = ticks_per_beat(meter.denominator);
        if position.beat > meter.numerator || position.tick as i64 >= beat_ticks {
            return Err(TimingError::InvalidPosition);
        }
        let bar_start = self.bar_start_tick(position.bar);
        Ok(bar_start + (position.beat as i64 - 1) * beat_ticks + position.tick as i64)
    }

    pub fn ticks_to_position(&self, tick: i64) -> MusicalPosition {
        let mut bar = 1;
        let mut start = 0_i64;
        if tick >= 0 {
            while start + self.ticks_in_bar(bar) <= tick {
                start += self.ticks_in_bar(bar);
                bar += 1;
            }
        } else {
            while start > tick {
                bar -= 1;
                start -= self.ticks_in_bar(bar);
            }
        }
        let offset = tick - start;
        let beat_ticks = ticks_per_beat(self.meter_at_bar(bar).denominator);
        MusicalPosition::new(
            bar,
            (offset / beat_ticks + 1) as u16,
            (offset % beat_ticks) as u32,
        )
    }

    pub fn position_to_seconds(&self, position: MusicalPosition) -> Result<f64, TimingError> {
        self.ticks_to_seconds(self.position_to_ticks(position)?)
    }

    pub fn seconds_to_position(&self, seconds: f64) -> Result<MusicalPosition, TimingError> {
        Ok(self.ticks_to_position(self.seconds_to_ticks(seconds)?))
    }

    pub fn position_to_samples(
        &self,
        position: MusicalPosition,
        sample_rate: u32,
    ) -> Result<i64, TimingError> {
        if sample_rate == 0 {
            return Err(TimingError::InvalidSampleRate);
        }
        Ok((self.position_to_seconds(position)? * sample_rate as f64).round() as i64)
    }

    pub fn samples_to_position(
        &self,
        samples: i64,
        sample_rate: u32,
    ) -> Result<MusicalPosition, TimingError> {
        if sample_rate == 0 {
            return Err(TimingError::InvalidSampleRate);
        }
        self.seconds_to_position(samples as f64 / sample_rate as f64)
    }

    pub fn resolve_event_timing(
        &self,
        timing: &EventTiming,
        sections: &[SongSection],
    ) -> Result<i64, TimingError> {
        match timing {
            EventTiming::Musical { position } => self.position_to_ticks(*position),
            EventTiming::Seconds { seconds } => self.seconds_to_ticks(*seconds),
            EventTiming::SectionBoundary { section_id } => self.section_tick(section_id, sections),
            EventTiming::BarsBeforeSection { section_id, bars } => {
                let section = sections
                    .iter()
                    .find(|s| &s.id == section_id)
                    .ok_or(TimingError::InvalidPosition)?;
                self.position_to_ticks(MusicalPosition::new(
                    section.start.bar - *bars as i32,
                    section.start.beat,
                    section.start.tick,
                ))
            }
            EventTiming::BeatsBeforeSection { section_id, beats } => {
                Ok(self.section_tick(section_id, sections)? - *beats as i64 * PPQ as i64)
            }
        }
    }

    fn section_tick(&self, id: &str, sections: &[SongSection]) -> Result<i64, TimingError> {
        let section = sections
            .iter()
            .find(|s| s.id == id)
            .ok_or(TimingError::InvalidPosition)?;
        self.position_to_ticks(section.start)
    }

    fn meter_at_bar(&self, bar: i32) -> MeterChange {
        self.meters
            .iter()
            .rev()
            .find(|m| m.position.bar <= bar)
            .copied()
            .unwrap_or(self.meters[0])
    }

    fn ticks_in_bar(&self, bar: i32) -> i64 {
        let meter = self.meter_at_bar(bar);
        meter.numerator as i64 * ticks_per_beat(meter.denominator)
    }

    fn bar_start_tick(&self, target: i32) -> i64 {
        let mut tick = 0;
        if target >= 1 {
            for bar in 1..target {
                tick += self.ticks_in_bar(bar);
            }
        } else {
            for bar in target..1 {
                tick -= self.ticks_in_bar(bar);
            }
        }
        tick
    }

    fn ticks_to_seconds(&self, target: i64) -> Result<f64, TimingError> {
        if target == 0 {
            return Ok(0.0);
        }
        if target > 0 {
            Ok(self.integrate_forward(0, target))
        } else {
            Ok(-self.integrate_forward(target, 0))
        }
    }

    fn integrate_forward(&self, start: i64, end: i64) -> f64 {
        let mut cursor = start;
        let mut seconds = 0.0;
        while cursor < end {
            let index = self.tempo_index_at(cursor);
            let point = &self.tempos[index];
            let next = self.tempos.get(index + 1);
            let boundary = next.map(|p| p.tick).unwrap_or(end).min(end);
            if point.transition == TempoTransition::Linear {
                if let Some(next) = next {
                    seconds += ramp_seconds(point, next, cursor, boundary);
                } else {
                    seconds += constant_seconds(boundary - cursor, point.bpm);
                }
            } else {
                seconds += constant_seconds(boundary - cursor, point.bpm);
            }
            cursor = boundary;
        }
        seconds
    }

    fn seconds_to_ticks(&self, seconds: f64) -> Result<i64, TimingError> {
        if !seconds.is_finite() {
            return Err(TimingError::InvalidTime);
        }
        if seconds == 0.0 {
            return Ok(0);
        }
        let direction = if seconds > 0.0 { 1_i64 } else { -1_i64 };
        let mut low = 0_i64;
        let mut high = direction * PPQ as i64;
        while (self.ticks_to_seconds(high)? - seconds) * (direction as f64) < 0.0 {
            low = high;
            high = high.saturating_mul(2);
        }
        if direction < 0 {
            std::mem::swap(&mut low, &mut high);
        }
        while high - low > 1 {
            let mid = low + (high - low) / 2;
            if self.ticks_to_seconds(mid)? < seconds {
                low = mid;
            } else {
                high = mid;
            }
        }
        let low_error = (self.ticks_to_seconds(low)? - seconds).abs();
        let high_error = (self.ticks_to_seconds(high)? - seconds).abs();
        Ok(if low_error <= high_error { low } else { high })
    }

    fn tempo_index_at(&self, tick: i64) -> usize {
        self.tempos
            .partition_point(|p| p.tick <= tick)
            .saturating_sub(1)
    }
}

fn ticks_per_beat(denominator: u16) -> i64 {
    PPQ as i64 * 4 / denominator as i64
}
fn constant_seconds(ticks: i64, bpm: f64) -> f64 {
    ticks as f64 * 60.0 / (PPQ as f64 * bpm)
}

fn ramp_seconds(start: &TempoPoint, end: &TempoPoint, from: i64, to: i64) -> f64 {
    let length = (end.tick - start.tick) as f64;
    let slope = (end.bpm - start.bpm) / length;
    let x0 = (from - start.tick) as f64;
    let x1 = (to - start.tick) as f64;
    if slope.abs() < 1e-12 {
        constant_seconds(to - from, start.bpm)
    } else {
        60.0 / (PPQ as f64 * slope) * ((start.bpm + slope * x1) / (start.bpm + slope * x0)).ln()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    fn pos(bar: i32, beat: u16, tick: u32) -> MusicalPosition {
        MusicalPosition::new(bar, beat, tick)
    }
    fn meter(bar: i32, n: u16, d: u16) -> MeterChange {
        MeterChange {
            position: pos(bar, 1, 0),
            numerator: n,
            denominator: d,
        }
    }
    fn tempo(p: MusicalPosition, bpm: f64, transition: TempoTransition) -> TempoChange {
        TempoChange {
            position: p,
            bpm,
            transition,
        }
    }
    fn fixed(n: u16, d: u16, bpm: f64) -> Timeline {
        Timeline::new(
            vec![meter(1, n, d)],
            vec![tempo(pos(1, 1, 0), bpm, TempoTransition::Instant)],
        )
        .unwrap()
    }

    #[test]
    fn four_four_at_120() {
        assert_abs_diff_eq!(
            fixed(4, 4, 120.0)
                .position_to_seconds(pos(2, 1, 0))
                .unwrap(),
            2.0
        );
    }

    #[test]
    fn three_four_at_90() {
        assert_abs_diff_eq!(
            fixed(3, 4, 90.0).position_to_seconds(pos(2, 1, 0)).unwrap(),
            2.0
        );
    }

    #[test]
    fn tempo_change_on_bar_boundary() {
        let t = Timeline::new(
            vec![meter(1, 4, 4)],
            vec![
                tempo(pos(1, 1, 0), 120.0, TempoTransition::Instant),
                tempo(pos(2, 1, 0), 60.0, TempoTransition::Instant),
            ],
        )
        .unwrap();
        assert_abs_diff_eq!(t.position_to_seconds(pos(3, 1, 0)).unwrap(), 6.0);
    }

    #[test]
    fn tempo_change_mid_bar() {
        let t = Timeline::new(
            vec![meter(1, 4, 4)],
            vec![
                tempo(pos(1, 1, 0), 120.0, TempoTransition::Instant),
                tempo(pos(1, 3, 0), 60.0, TempoTransition::Instant),
            ],
        )
        .unwrap();
        assert_abs_diff_eq!(t.position_to_seconds(pos(2, 1, 0)).unwrap(), 3.0);
    }

    #[test]
    fn exact_linear_tempo_ramp() {
        let t = Timeline::new(
            vec![meter(1, 4, 4)],
            vec![
                tempo(pos(1, 1, 0), 60.0, TempoTransition::Linear),
                tempo(pos(2, 1, 0), 120.0, TempoTransition::Instant),
            ],
        )
        .unwrap();
        assert_abs_diff_eq!(
            t.position_to_seconds(pos(2, 1, 0)).unwrap(),
            4.0 * 2.0_f64.ln(),
            epsilon = 1e-12
        );
    }

    #[test]
    fn pickup_before_bar_one() {
        let t = fixed(4, 4, 120.0);
        assert_abs_diff_eq!(t.position_to_seconds(pos(0, 4, 0)).unwrap(), -0.5);
        assert_eq!(t.seconds_to_position(-0.5).unwrap(), pos(0, 4, 0));
    }

    #[test]
    fn round_trips_position_seconds_and_samples() {
        let t = fixed(4, 4, 123.0);
        let p = pos(7, 3, 480);
        let seconds = t.position_to_seconds(p).unwrap();
        assert_eq!(t.seconds_to_position(seconds).unwrap(), p);
        for rate in [44_100, 48_000, 96_000] {
            let samples = t.position_to_samples(p, rate).unwrap();
            let restored = t.samples_to_position(samples, rate).unwrap();
            assert!(
                (t.position_to_ticks(restored).unwrap() - t.position_to_ticks(p).unwrap()).abs()
                    <= 1
            );
        }
    }

    #[test]
    fn time_signature_change() {
        let t = Timeline::new(
            vec![meter(1, 4, 4), meter(3, 3, 4)],
            vec![tempo(pos(1, 1, 0), 120.0, TempoTransition::Instant)],
        )
        .unwrap();
        assert_abs_diff_eq!(t.position_to_seconds(pos(4, 1, 0)).unwrap(), 5.5);
        assert_eq!(t.seconds_to_position(5.5).unwrap(), pos(4, 1, 0));
    }
}
