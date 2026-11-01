use wprs_scene::{Blend, Kind, Orient, Render, Scene, Spread, Stack, System};

pub(super) fn turn(r: &Render) -> [f32; 4] {
    let mode = match r.orient {
        Some(Orient::Velocity) => 1.0,
        Some(Orient::Fixed) => 2.0,
        _ => 0.0,
    };
    [mode, r.angle.unwrap_or(0.0).to_radians(), 0.0, 0.0]
}

pub(super) fn far(scene: &Scene, id: &str, s: Option<&System>) -> Option<(Stack, f32)> {
    let s = s.filter(|s| s.perspective.is_some())?;
    let drawn = |l: &wprs_scene::Layer| matches!(&l.kind, Kind::Particles { system, .. } if system == id);
    scene.layers().any(|l| drawn(l) && l.blend == Blend::Normal).then_some(())?;
    let reach = |d: &Option<Spread<f32>>| d.as_ref().map_or(0.0, |d| d.ends[0].abs().max(d.ends[1].abs()));
    let life = s.init.life.ends[0].max(s.init.life.ends[1]);
    Some((Stack::Farthest, (reach(&s.init.depth) + reach(&s.init.depth_speed) * life).max(1.0)))
}
