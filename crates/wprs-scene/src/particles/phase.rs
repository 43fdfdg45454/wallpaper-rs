use super::{MAX_EMITTERS2, MAX_REMAPS, Sim};

const PERIOD: f64 = 4096.0;

fn wrap(v: f64, period: f64) -> f32 {
    v.rem_euclid(period) as f32
}

pub fn runtime(row: [[f32; 4]; 3], t: f64) -> f32 {
    let (lo, hi) = (f64::from(row[0][1]), f64::from(row[0][2]));
    let span = (hi - lo).abs();
    if row[2][1] > 0.5 {
        return t.clamp(lo.min(hi) - 1.0, lo.max(hi) + 1.0) as f32;
    }
    match row[2][0] as u32 {
        1 if span > 0.0 => (lo + (t - lo).rem_euclid(span)) as f32,
        2 if span > 0.0 => (lo + (t - lo).rem_euclid(span * PERIOD)) as f32,
        _ => t as f32,
    }
}

impl Sim {
    pub fn phases(&mut self, t: f64) {
        for e in 0..(self.counts[1] as usize).min(MAX_EMITTERS2) {
            match self.shapes[e] {
                6 => self.emitters[4 * e + 1][3] = wrap(f64::from(self.emitters[4 * e + 1][3]) * t, 360.0),
                7 if self.emitters[4 * e + 1][3] > 0.5 => {
                    self.paths[2 * e + 1][0] = wrap(f64::from(self.paths[2 * e + 1][0]) * t, 1.0)
                }
                _ => {}
            }
        }
        if self.fields[1][3] > 0.5 {
            self.fields[0][2] = wrap(f64::from(self.fields[0][2]) * t, PERIOD);
        }
        if self.fields[3][3] > 0.5 {
            self.fields[2][3] = wrap(f64::from(self.fields[2][3]) * t, PERIOD);
        }
        for k in 0..(self.remap[0] as usize).min(MAX_REMAPS) {
            let row = [self.remaps[3 * k], self.remaps[3 * k + 1], self.remaps[3 * k + 2]];
            if row[0][0] as u32 == 1 {
                self.remaps[3 * k][3] = runtime(row, t);
            }
        }
        for k in 0..(self.counts[2] as usize).min(16) {
            let (f, a, b) = (self.forces[4 * k], self.forces[4 * k + 1], self.forces[4 * k + 2]);
            if f[0] as u32 == 15 && f[1] as u32 == 1 {
                let bits = b[3] as u32;
                let clamps = [(bits & 1) as f32, (bits >> 1 & 1) as f32];
                let row =
                    [[f[1], a[0], a[1], f[2]], [b[0], a[2], a[3], b[1]], [b[2], clamps[0], clamps[1], f[3]]];
                self.forces[4 * k][2] = runtime(row, t);
            }
        }
    }
}
