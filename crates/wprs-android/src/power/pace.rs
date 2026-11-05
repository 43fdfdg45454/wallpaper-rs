const SECOND_NS: i64 = 1_000_000_000;

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Pace {
    last: i64,
    interval: i64,
    vsync: i64,
}

impl Pace {
    pub(crate) fn wait(&mut self, nanos: i64, fps: u32, rate: u32) -> Option<i64> {
        let paced = if rate > 0 { fps.min(rate) } else { fps };
        (self.interval, self.vsync) = (SECOND_NS / i64::from(paced.max(1)), crate::power::vsync_ns());
        let ready = self.last + self.interval - self.vsync / 2;
        (self.last > 0 && nanos < ready).then(|| ((ready - nanos) / 1_000_000).max(1))
    }

    pub(crate) fn shown(&mut self, nanos: i64, presented: bool) {
        if presented {
            let ideal = self.last + self.interval;
            let near = ideal.clamp(nanos - self.vsync / 4, nanos + self.vsync / 4);
            self.last = if self.last > 0 { near } else { nanos };
        }
    }

    pub(crate) fn armed(&self, nanos: i64, rate: u32) -> i64 {
        (self.last + SECOND_NS / i64::from(rate.max(1)) - self.vsync / 2 - nanos) / 1_000_000
    }
}

#[cfg(test)]
mod tests {
    use super::Pace;

    fn run(hz: f64, fps: u32, draw_ms: f64) -> f64 {
        let vsync = (1e9 / hz) as i64;
        let (mut pace, mut arm, mut shown) = (Pace::default(), 0_i64, 0_u32);
        crate::power::refresh(hz as f32);
        for tick in 1..=(hz as i64 * 4) {
            let now = tick * vsync;
            if now < arm {
                continue;
            }
            match pace.wait(now, fps, fps) {
                Some(wait) => arm = now + wait * 1_000_000,
                None => {
                    pace.shown(now, true);
                    shown += 1;
                    let done = now + (draw_ms * 1e6) as i64;
                    arm = (now + pace.armed(now, fps).max(1) * 1_000_000).max(done);
                }
            }
        }
        f64::from(shown) / 4.0
    }

    #[test]
    fn frames_keep_their_rate_whatever_the_draw_time_and_panel() {
        for (hz, fps) in
            [(60.0, 60), (60.0, 30), (60.0, 20), (120.0, 60), (120.0, 30), (90.0, 60), (90.0, 30)]
        {
            for draw_ms in [1.0_f64, 5.0, 8.0, 12.0] {
                let got = run(hz, fps, draw_ms.min(1000.0 / hz - 1.0));
                assert!(
                    (got - f64::from(fps)).abs() <= 1.0,
                    "{hz} Hz at {fps} fps drawing {draw_ms} ms gave {got}"
                );
            }
        }
        crate::power::refresh(60.0);
    }
}
