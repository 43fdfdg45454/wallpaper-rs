use std::f32::consts::TAU;

const STILL: f32 = 1e-3;
const RECENTER_S: f32 = 4.0;
const TILT_GAIN: f32 = 4.0;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Spring {
    pub at: [f32; 2],
    pub speed: [f32; 2],
}

impl Spring {
    pub fn step(&mut self, goal: [f32; 2], smoothing_s: f32, dt: f32) -> bool {
        let before = self.at;
        let w = 2.0 / smoothing_s.max(1e-4);
        let e = (-w * dt.max(0.0)).exp();
        let axis = |i: usize| {
            let (d, c) = (self.at[i] - goal[i], self.speed[i] + w * (self.at[i] - goal[i]));
            (goal[i] + (d + c * dt) * e, (c - w * (d + c * dt)) * e)
        };
        let [x, y] = [axis(0), axis(1)];
        (self.at, self.speed) = ([x.0, y.0], [x.1, y.1]);
        if self.settled(goal) {
            (self.at, self.speed) = (goal, [0.0; 2]);
        }
        self.at != before
    }

    pub fn settled(&self, goal: [f32; 2]) -> bool {
        (0..2).all(|i| (self.at[i] - goal[i]).abs() < STILL && self.speed[i].abs() < STILL * 10.0)
    }
}

pub fn orbit(seconds: f32) -> [f32; 2] {
    [(seconds * TAU / 17.0).sin(), 0.7 * (seconds * TAU / 23.0 + 0.6).sin()]
}

#[derive(Debug, Clone, Default)]
pub struct Tilt {
    rest: Option<[f32; 3]>,
}

impl Tilt {
    pub fn follow(&mut self, gravity: [f32; 3], dt: f32) -> [f32; 2] {
        let length = gravity.iter().map(|v| v * v).sum::<f32>().sqrt();
        if length < 1e-6 {
            return [0.0; 2];
        }
        let g = gravity.map(|v| v / length);
        let rest = self.rest.get_or_insert(g);
        let k = (dt / RECENTER_S).clamp(0.0, 1.0);
        (0..3).for_each(|i| rest[i] += (g[i] - rest[i]) * k);
        [0, 1].map(|i| ((g[i] - rest[i]) * TILT_GAIN).clamp(-1.0, 1.0))
    }
}

pub fn mount_matrix(text: &str) -> Option<[[f32; 3]; 3]> {
    let rows: Vec<[f32; 3]> = text
        .trim()
        .split(';')
        .map(|row| {
            let values: Option<Vec<f32>> = row.split(',').map(|v| v.trim().parse().ok()).collect();
            values?.try_into().ok()
        })
        .collect::<Option<_>>()?;
    rows.try_into().ok()
}

pub fn gravity(raw: [f32; 3], scale: f32, matrix: &[[f32; 3]; 3]) -> [f32; 3] {
    matrix.map(|row| (0..3).map(|i| row[i] * raw[i] * scale).sum())
}
