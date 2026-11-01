use super::points::{Room, turn};
use super::sow::input;
use super::{EmitShape, MAX_CONTROL, MAX_EMITTERS2, Sim, System, Target};
use crate::resolve::Context;
use crate::value::{Band, Level};
use crate::{Affine, Frame};

pub(super) struct Fit<'a> {
    pub frame: Affine,
    pub to_sim: Affine,
    pub room: &'a Room,
    pub hatched: bool,
    pub draws: &'a Frame<'a>,
    pub dt: f32,
}

fn heard(cx: &Context, b: Band) -> f32 {
    let a = cx.audio;
    let mean = |lo: u32, hi: u32| {
        let it = a.average.iter().skip(lo as usize).take(hi.saturating_sub(lo) as usize + 1);
        it.sum::<f32>() / (hi.saturating_sub(lo) + 1) as f32
    };
    match b {
        Band::Index(i) => mean(i, i),
        Band::Range([lo, hi]) => mean(lo, hi),
        Band::Named(Level::Bass) => a.bass,
        Band::Named(Level::Mid) => a.mid,
        Band::Named(Level::Treble) => a.treble,
        Band::Named(Level::Level) => a.level,
        Band::Named(Level::Beat) => f32::from(a.beat),
    }
}

fn level(cx: &Context, band: &str) -> f32 {
    let named = match band {
        "bass" => Level::Bass,
        "mid" => Level::Mid,
        "treble" => Level::Treble,
        "beat" => Level::Beat,
        "level" => Level::Level,
        n => return heard(cx, Band::Index(n.parse().unwrap_or(0))),
    };
    heard(cx, Band::Named(named))
}

fn spot(s: &System, t: Option<&super::Target>, f: &Fit, cx: &Context) -> [f32; 3] {
    match (t, f.hatched) {
        (Some(Target::Point(p)), true) => [p[0], p[1], 0.0],
        (Some(Target::Control(h)), true) => {
            let at =
                s.points.get(usize::from(h.0)).and_then(|p| p.at.as_ref()).map_or([0.0; 2], |v| v.get(cx).0);
            [at[0], at[1], 0.0]
        }
        (_, true) => [0.0; 3],
        (Some(Target::Control(h)), false) => f.room.points[usize::from(h.0).min(MAX_CONTROL - 1)],
        (Some(t), false) => {
            let [x, y, _] = f.room.aim(t);
            [x, y, if matches!(t, Target::Point(_)) { turn(f.frame) } else { 0.0 }]
        }
        (None, false) => f.room.points[0],
    }
}

pub(super) fn fit(sim: &mut Sim, s: &System, f: &Fit, cx: &Context, moves: &mut [[f32; 6]; MAX_EMITTERS2]) {
    let grow = (f.frame.0[0] * f.frame.0[3] - f.frame.0[1] * f.frame.0[2]).abs().sqrt();
    for (e, em) in s.emitters.iter().take(MAX_EMITTERS2).enumerate() {
        let a = &mut sim.emitters[4 * e];
        match &em.shape {
            EmitShape::Around(r) => {
                let [x, y, angle] = spot(s, r.at.as_ref(), f, cx);
                let scale = if f.hatched { 1.0 } else { grow };
                (a[0], a[1], a[2]) = (x, y, a[2] * scale);
                sim.emitters[4 * e + 1][1] = angle;
            }
            EmitShape::Between(b) => {
                let ([fx, fy, _], [tx, ty, _]) = (spot(s, Some(&b.from), f, cx), spot(s, Some(&b.to), f, cx));
                *a = [fx, fy, tx, ty];
            }
            EmitShape::Layer(l) => {
                let Some(d) = f.draws.draws.iter().find(|d| d.layer.id == l.id) else { continue };
                let m = f.to_sim.then(d.world).0;
                let last = std::mem::replace(&mut moves[e], m);
                let last = if last == [0.0; 6] { m } else { last };
                let v: [f32; 6] =
                    std::array::from_fn(|i| if f.dt > 0.0 { (m[i] - last[i]) / f.dt } else { 0.0 });
                sim.emitters[4 * e + 2] = [m[0], m[1], m[2], m[3]];
                sim.emitters[4 * e + 3] = [m[4], m[5], 0.0, 0.0];
                sim.paths[2 * e] = [v[0], v[1], v[2], v[3]];
                sim.paths[2 * e + 1] = [v[4], v[5], 0.0, 0.0];
                continue;
            }
            _ => continue,
        }
        if !f.hatched {
            sim.emitters[4 * e + 2] = [1.0, 0.0, 0.0, 1.0];
            sim.emitters[4 * e + 3] = [0.0; 4];
        }
    }
    let home = f.to_sim.apply([cx.canvas[0] / 2.0, cx.canvas[1] / 2.0]);
    (sim.depth[2][1], sim.depth[2][2]) = (home[0], home[1]);
    if let Some(band) = s.init.turbulent.and_then(|t| t.audio) {
        sim.fields[1][2] *= heard(cx, band);
    }
    for (k, r) in s.init.remap.iter().take(super::MAX_REMAPS).enumerate() {
        if let Some((x, y)) = live(cx, f.room, &r.input) {
            (sim.remaps[3 * k][3], sim.remaps[3 * k + 2][3]) = (x, y);
        }
    }
}

pub(super) fn live(cx: &Context, room: &Room, name: &str) -> Option<(f32, f32)> {
    let [kind, point] = input(name);
    Some(match kind as u32 {
        0 => (cx.time_of_day("day"), 0.0),
        6 => {
            let [x, y, _] = room.points[(point as usize).min(MAX_CONTROL - 1)];
            (x, y)
        }
        7 => (level(cx, name.trim_start_matches("audio:")), 0.0),
        8 | 9 => {
            let p = cx.pointer;
            (if p.inside { p.smooth[kind as usize - 8] } else { 0.5 }, 0.0)
        }
        _ => return None,
    })
}
