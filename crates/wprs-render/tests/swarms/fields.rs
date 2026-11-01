use super::*;
use wprs_scene::Particle;

pub(super) fn system(shape: Value, init: Value, more: Value) -> Value {
    let mut s = json!({ "capacity": 512, "seed": 5, "emitters": [{ "shape": shape, "rate": 240 }],
        "init": { "life": [20, 20], "speed": [0, 0], "size": [3, 3], "color": ["#ffffff", "#ffffff"] } });
    init.as_object().into_iter().flatten().for_each(|(k, v)| s["init"][k] = v.clone());
    more.as_object().into_iter().flatten().for_each(|(k, v)| s[k] = v.clone());
    s
}

pub(super) fn alive(r: &Renderer, run: &Run) -> Vec<Particle> {
    let all = r.particles(&run.view).unwrap();
    let mut out: Vec<Particle> =
        all.into_iter().flat_map(|(_, p)| p).flatten().filter(|p| p.age < p.life).collect();
    out.sort_by_key(|p| p.id);
    out
}

pub(super) fn both(
    r: &Renderer,
    systems: &Value,
    layers: &Value,
    frames: usize,
) -> Option<[Vec<Particle>; 2]> {
    let mut out = [Vec::new(), Vec::new()];
    for (k, gpu) in [false, true].into_iter().enumerate() {
        let mut run = Run::new(r, systems.clone(), layers.clone(), gpu)?;
        (0..frames).for_each(|_| drop(run.frame(r, 1.0 / 30.0)));
        out[k] = alive(r, &run);
    }
    Some(out)
}

pub(super) fn agree(name: &str, [cpu, gpu]: &[Vec<Particle>; 2], tol: f32) {
    assert!(!cpu.is_empty(), "{name}: nothing was born");
    assert_eq!(cpu.len(), gpu.len(), "{name}: as many particles live on both paths");
    let near = |a: f32, b: f32, tol: f32| (a - b).abs() <= tol * (1.0 + a.abs().max(b.abs()) * 1e-3);
    for (c, g) in cpu.iter().zip(gpu) {
        assert_eq!(c.id, g.id, "{name}");
        let pairs = [(c.pos[0], g.pos[0]), (c.pos[1], g.pos[1]), (c.vel[0], g.vel[0]), (c.vel[1], g.vel[1])];
        let pairs =
            pairs.into_iter().chain([(c.size, g.size), (c.depth[0], g.depth[0]), (c.depth[1], g.depth[1])]);
        for (a, b) in pairs {
            assert!(near(a, b, tol), "{name}: {c:?} and {g:?}");
        }
        for (a, b) in c.color.iter().zip(g.color) {
            assert!(near(*a, b, 0.01), "{name}: {c:?} and {g:?}");
        }
    }
}

const LAYER: &str = r#"[{ "id": "p", "type": "particles", "system": "s" }]"#;

fn layers() -> Value {
    serde_json::from_str(LAYER).unwrap()
}

#[test]
fn shapes_around_and_between_points_agree_on_both_paths() {
    let Some(r) = renderer() else { return };
    let around =
        json!({ "around": { "at": [80, 40], "count": 12, "radius": 20, "mode": "mirror", "speed": 30 } });
    let between = json!({ "between": { "from": "p1", "to": [140, 60], "count": 16,
        "arc": { "amount": 0.3, "side": 0 }, "taper": { "position": 0.5, "size": 0.2, "speed": 0.5 } } });
    for (name, shape) in [("around", around), ("between", between)] {
        let systems = json!({ "s": system(shape, json!({}), json!({ "points": [{ "at": [0, 0] }, { "at": [20, 20] }] })) });
        let Some(found) = both(r, &systems, &layers(), 8) else { return };
        agree(name, &found, 0.05);
        if name == "around" {
            let far = found[0].iter().map(|p| (p.pos[0] - 80.0).hypot(p.pos[1] - 40.0)).fold(0.0, f32::max);
            let near = found[0].iter().map(|p| (p.pos[0] - 80.0).hypot(p.pos[1] - 40.0)).fold(99.0, f32::min);
            assert!(far < 20.1 && near > 19.9, "births sit on the ring: {near} to {far}");
        }
    }
}

#[test]
fn turbulent_starts_fractal_offsets_remaps_and_depth_agree_on_both_paths() {
    let Some(r) = renderer() else { return };
    let box_ = json!({ "box": [[20, 20], [140, 60]] });
    let remap = json!([{ "input": "random", "in": [0, 1], "output": "size", "out": [1, 8] },
        { "input": "distance:p1", "in": [0, 100], "clamp_in": true, "output": "alpha", "out": [1, 0], "wave": "sine" },
        { "input": "runtime", "in": [0, 1], "output": "speed", "out": [0, 30], "op": "add" },
        { "input": "life", "in": [0, 40], "output": "color", "out": [0.2, 1], "op": "mul", "clamp_out": true }]);
    let cases = [
        (
            "turbulent",
            json!({ "turbulent": { "forward": 90, "scale": 0.5, "noise_speed": 2, "speed": [20, 40], "offset": 3 } }),
            json!({}),
        ),
        (
            "offset",
            json!({ "offset_noise": { "distance": 15, "scale": 2, "octaves": 4, "noise_scale": 1, "noise_speed": 1, "sign": [0, 1] } }),
            json!({}),
        ),
        ("remap", json!({ "remap": remap }), json!({ "points": [{ "at": [0, 0] }, { "at": [60, 30] }] })),
        (
            "depth",
            json!({ "depth": [-50, 150], "depth_speed": [-20, 20], "speed": [10, 20] }),
            json!({ "perspective": { "focal": 200 } }),
        ),
    ];
    for (name, init, more) in cases {
        let systems = json!({ "s": system(box_.clone(), init, more) });
        let Some(found) = both(r, &systems, &layers(), 8) else { return };
        agree(name, &found, if matches!(name, "turbulent" | "offset") { 0.3 } else { 0.05 });
    }
}
