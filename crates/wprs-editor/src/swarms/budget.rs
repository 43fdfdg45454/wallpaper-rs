use super::shapes::Frame;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use wprs_scene::{Bake, Cloud, Quality, Runtime, Scene, Sim, Tier, effective, flocks};

const STEP: f64 = 1.0 / 30.0;
const MOST: f64 = 10.0;
const SHOWN: usize = 300;

struct Ran {
    bake: Bake,
    clouds: Vec<Cloud>,
}

fn loaded(dir: &Path, doc: &Value, system: &str, layer: Option<&str>) -> Option<(Scene, usize)> {
    let mut scene = Scene::from_json(&doc["scene"].to_string()).ok()?;
    drop(crate::cache::read_files(&mut scene, dir));
    let index = scene.particles.keys().position(|k| k == system)?;
    let all = flocks(&scene);
    let own = all.iter().position(|f| f.system == index && f.layer.as_deref() == layer);
    let flock = own.or_else(|| all.iter().position(|f| f.system == index))?;
    Some((scene, flock))
}

fn ran(dir: &Path, doc: &Value, (scene, flock): (Scene, usize), t: f64, q: Quality) -> Option<Ran> {
    let props = crate::meta::read(doc, Some(dir)).map(|m| m.properties).unwrap_or_default();
    let view = [0.0, 0.0, scene.canvas.size[0] as f32, scene.canvas.size[1] as f32];
    let bake = Bake::new(&scene, flock, Tier::T3, true, |_| None)?;
    let mut rt = Runtime::new(Arc::new(scene), effective(&props, &[]));
    let mut clouds: Vec<Cloud> = bake.allocs.iter().map(|a| Cloud::new(*a)).collect();
    let (mut sims, mut stamp) = (vec![Sim::default(); clouds.len()], 0);
    for _ in 0..((t.clamp(0.0, MOST) / STEP).ceil() as usize).max(1) {
        rt.advance(STEP);
        rt.frame(view, q);
        let Some(swarm) = rt.swarm(flock) else { continue };
        if std::mem::replace(&mut stamp, swarm.stamp) == swarm.stamp {
            continue;
        }
        for j in 0..swarm.count() {
            (0..sims.len()).for_each(|p| sims[p] = bake.apply(p, swarm.sim(j, p)));
            Cloud::brood(&mut clouds, &sims, &bake.hosts, &bake.points);
        }
    }
    clouds[0].strand(&sims[0]);
    Some(Ran { bake, clouds })
}

fn tiers(scene: &Scene, flock: usize) -> Value {
    let rows = [Tier::T0, Tier::T1, Tier::T2, Tier::T3].map(|tier| {
        let slots = |cpu| Bake::new(scene, flock, tier, cpu, |_| None).map_or(0, |b| b.allocs[0]);
        let trail = Bake::new(scene, flock, tier, false, |_| None).map_or(0, |b| b.trails[0]);
        json!({ "tier": tier, "gpu": slots(false), "cpu": slots(true), "trail": trail,
            "neighbours": tier.neighbours(), "rope": tier.rope() })
    });
    json!(rows)
}

fn levels(s: &wprs_scene::System, kept: u32) -> Value {
    let long = s.render.trail.as_ref().is_some_and(wprs_scene::Trail::long);
    let rope = s.render.rope.map_or(0, |r| r.subdivide);
    let rows = [Quality::Q0, Quality::Q1, Quality::Q2, Quality::Q3].map(|q| {
        let trail = if long { q.long_trail(kept) } else { q.trail(kept) };
        json!({ "quality": q, "births": q.particles(), "neighbours": q.neighbours(), "trail": trail,
            "subdivide": q.subdivide(rope) })
    });
    json!(rows)
}

pub fn budget(dir: &Path, doc: &Value, system: &str, layer: Option<&str>, t: f64, q: Quality) -> Value {
    let Some((scene, flock)) = loaded(dir, doc, system, layer) else { return Value::Null };
    let Some(s) = scene.particles.get(system).cloned() else { return Value::Null };
    let (rows, kept) = (
        tiers(&scene, flock),
        Bake::new(&scene, flock, Tier::T3, false, |_| None).map_or(0, |b| b.trails[0]),
    );
    let world = s.space == wprs_scene::Placement::World;
    let layer = layer.or_else(|| super::handles::drawn_by(doc, system));
    let f = if world {
        Frame::new([1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
    } else {
        super::handles::frame(dir, doc, layer, t)
    };
    let at = |p: [f32; 2]| f.round(f.apply([f64::from(p[0]), f64::from(p[1])]));
    let mut out = json!({ "system": system, "capacity": s.capacity, "time": t.clamp(0.0, MOST), "quality": q,
        "tiers": rows, "levels": levels(&s, kept), "cpu_boids": wprs_scene::CPU_BOIDS, "max_rope": wprs_scene::MAX_ROPE,
        "boids": s.forces.iter().any(|o| matches!(o.force, wprs_scene::Force::Boids(_))),
        "alive": 0, "slots": 0, "particles": [], "rope": [], "trails": [] });
    let Some(r) = ran(dir, doc, (scene, flock), t, q) else { return out };
    let alive: Vec<_> = r.clouds[0].particles.iter().enumerate().filter(|(_, p)| p.age < p.life).collect();
    out["alive"] = json!(r.clouds.iter().map(Cloud::alive).sum::<u32>());
    out["slots"] = json!(r.bake.alloc());
    let shown = alive.iter().step_by(alive.len().div_ceil(SHOWN).max(1));
    out["particles"] = json!(
        shown
            .map(|(_, p)| {
                let [x, y] = at(p.pos);
                json!([x, y, p.id])
            })
            .collect::<Vec<_>>()
    );
    out["rope"] = json!(
        r.clouds[0].rope.iter().take_while(|s| s[7] > 0.5).map(|s| at([s[0], s[1]])).collect::<Vec<_>>()
    );
    let k = r.bake.trails[0] as usize;
    let marks = |i: usize| r.clouds[0].trail[i * k..(i + 1) * k].iter().filter(|m| m[2] & 0xffff != 0);
    let trails = alive.iter().take(12).filter(|_| k > 1).map(|(i, _)| {
        marks(*i).map(|m| at([f32::from_bits(m[0]), f32::from_bits(m[1])])).collect::<Vec<_>>()
    });
    out["trails"] = json!(trails.collect::<Vec<_>>());
    out
}
