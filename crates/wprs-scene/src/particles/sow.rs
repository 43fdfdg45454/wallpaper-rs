use super::seed::{RemapOp, RemapOutput, Wave};
use super::{Around, Between, Remap, Sim, Source, Spread, Sweep, System};

fn sweep(m: Sweep) -> f32 {
    f32::from(u8::from(m == Sweep::Mirror))
}

pub(super) fn around(a: &Around) -> [f32; 4] {
    [a.count.max(1) as f32, 0.0, sweep(a.mode), a.speed.0]
}

pub(super) fn between(b: &Between) -> [f32; 4] {
    [b.count.max(1) as f32, b.span.0, sweep(b.mode), f32::from(u8::from(b.taper.is_some()))]
}

pub(super) fn flags(l: &Source) -> f32 {
    f32::from(u8::from(l.color)) + 2.0 * f32::from(u8::from(l.inherit))
}

fn span(s: &Option<Spread<f32>>) -> [f32; 4] {
    s.as_ref().map_or([0.0; 4], |s| [s.ends[0], s.ends[1], s.exponent.unwrap_or(1.0), 1.0])
}

pub fn input(name: &str) -> [f32; 2] {
    let point = |h: &str| h.strip_prefix('p').and_then(|n| n.parse::<f32>().ok()).unwrap_or(0.0);
    match name {
        "daytime" => [0.0, 0.0],
        "runtime" => [1.0, 0.0],
        "life" => [2.0, 0.0],
        "age" => [3.0, 0.0],
        "speed" => [4.0, 0.0],
        "random" => [5.0, 0.0],
        "cursor:x" => [8.0, 0.0],
        "cursor:y" => [9.0, 0.0],
        n if n.starts_with("distance:") => [6.0, point(&n[9..])],
        _ => [7.0, 0.0],
    }
}

pub fn table(r: &Remap) -> [[f32; 4]; 3] {
    let [kind, _] = input(&r.input);
    let out = r.output as u8 as f32;
    let op = match r.op {
        RemapOp::Set => 0.0,
        RemapOp::Mul => 1.0,
        RemapOp::Add => 2.0,
        RemapOp::Sub => 3.0,
    };
    let wave = match r.wave {
        Wave::None => 0.0,
        Wave::Sine => 1.0,
        Wave::Noise => 2.0,
    };
    let flag = |b: bool| f32::from(u8::from(b));
    [
        [kind, r.from[0], r.from[1], 0.0],
        [out, r.to[0], r.to[1], op],
        [wave, flag(r.clamp_in), flag(r.clamp_out), 0.0],
    ]
}

impl Sim {
    pub(super) fn sow(&mut self, s: &System) {
        let i = &s.init;
        for (k, em) in s.emitters.iter().take(8).enumerate() {
            if let super::EmitShape::Between(b) = &em.shape {
                let (bow, taper) =
                    (b.arc.unwrap_or(super::Bow { amount: 0.0, side: 1.0 }), b.taper.unwrap_or_default());
                self.paths[2 * k] = [bow.amount, bow.side, taper.position, taper.size];
                self.paths[2 * k + 1] = [taper.speed, 0.0, 0.0, 0.0];
            }
        }
        if let Some(t) = &i.turbulent {
            self.fields[0] = [t.forward, t.scale, t.noise_speed, t.offset];
            self.fields[1] = [t.speed[0], t.speed[1], t.phase, 1.0];
        }
        if let Some(o) = &i.offset_noise {
            let [x, y] = o.sign.map(f32::from);
            self.fields[2] = [o.distance, o.scale, o.noise_scale, o.noise_speed];
            self.fields[3] = [x, y, o.octaves.clamp(1, 6) as f32, 1.0];
        }
        self.depth[0] = span(&i.depth);
        self.depth[1] = span(&i.depth_speed);
        if let Some(p) = s.perspective {
            self.depth[2] = [p.focal.max(1.0), self.clock[2] / 2.0, self.clock[3] / 2.0, 1.0];
        }
        let n = i.remap.len().min(super::MAX_REMAPS);
        self.remap[0] = n as u32;
        for (k, r) in i.remap.iter().take(n).enumerate() {
            self.remaps[3 * k..3 * k + 3].copy_from_slice(&table(r));
        }
    }
}

impl RemapOutput {
    pub fn all() -> [RemapOutput; 8] {
        use RemapOutput::*;
        [Size, Alpha, Color, Speed, Life, Rotation, Spin, Frame]
    }
}
