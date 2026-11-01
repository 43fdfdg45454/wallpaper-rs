use super::fields::{agree, alive, both, system};
use super::*;

fn layers() -> Value {
    json!([{ "id": "p", "type": "particles", "system": "s" }])
}

fn moving(more: Value) -> Value {
    let init = json!({ "life": [6, 6], "speed": [30, 60], "angle": [0, 360] });
    system(json!({ "box": [[30, 20], [130, 60]] }), init, more)
}

#[test]
fn keep_distance_keep_between_and_remap_operators_agree_on_both_paths() {
    let Some(r) = renderer() else { return };
    let cases = [
        (
            "keep distance",
            json!([{ "keep_distance": { "target": [80, 40], "distance": 20, "strength": 6 } }, { "drag": 1 }]),
        ),
        ("keep between", json!([{ "keep_between": { "from": [20, 10], "to": [140, 70], "strength": 8 } }])),
        (
            "remap operator",
            json!([{ "remap": { "input": "age", "in": [0, 1], "clamp_in": true, "output": "size", "out": [1, 6] } },
                { "remap": { "input": "distance:p0", "in": [0, 80], "output": "speed", "out": [40, 2], "op": "mul", "wave": "sine" } }]),
        ),
    ];
    for (name, forces) in cases {
        let systems = json!({ "s": moving(json!({ "forces": forces, "points": [{ "at": [80, 40] }] })) });
        let Some(found) = both(r, &systems, &layers(), 12) else { return };
        agree(name, &found, 0.05);
    }
}

#[test]
fn boids_agree_on_both_paths_while_neighbours_fit() {
    let Some(r) = renderer() else { return };
    let flock = json!({ "boids": { "radius": 12, "separation": 1, "alignment": 2, "cohesion": 0.5, "max_speed": 70 } });
    let mut s = moving(json!({ "forces": [flock] }));
    s["emitters"] = json!([{ "shape": { "box": [[10, 10], [150, 70]] }, "burst": { "count": 60 } }]);
    let Some(found) = both(r, &json!({ "s": s }), &layers(), 8) else { return };
    agree("boids", &found, 0.05);
}

#[test]
fn every_collider_and_response_agrees_on_both_paths() {
    let Some(r) = renderer() else { return };
    let walls = [
        json!({ "bounds": { "box": { "from": [40, 15], "to": [120, 65], "inside": true } }, "bounce": 0.6, "friction": 0.2 }),
        json!({ "bounds": { "box": { "from": [70, 30], "to": [90, 50] } }, "bounce": 1 }),
        json!({ "bounds": { "circle": { "center": [80, 40], "radius": 25, "inside": true } }, "stick": true }),
        json!({ "bounds": { "segment": { "from": [100, -50], "to": [100, 150] } }, "bounce": 0.5 }),
        json!({ "bounds": { "plane": { "point": [0, 60], "normal": [0, -1] } }, "kill": true }),
    ];
    for wall in walls {
        let systems =
            json!({ "s": moving(json!({ "forces": [{ "gravity": [0, 30] }], "collide": [wall] })) });
        let Some(found) = both(r, &systems, &layers(), 20) else { return };
        agree(&wall.to_string(), &found, 0.05);
        let stuck = |list: &[wprs_scene::Particle]| list.iter().filter(|p| p.spare[0] > 0.5).count();
        assert_eq!(stuck(&found[0]), stuck(&found[1]), "{wall}");
    }
}

#[test]
fn ropes_and_long_trails_share_their_geometry_on_both_paths() {
    let Some(r) = renderer() else { return };
    let render = json!({ "rope": { "subdivide": 3, "width": 4 }, "trail": { "length": 30, "width": 1, "segments": 6, "subdivide": 2 } });
    let init = json!({ "life": [3, 3], "speed": [40, 40], "angle": [10, 10] });
    let mut s = system(
        json!({ "point": [10, 20] }),
        init,
        json!({ "render": render, "forces": [{ "gravity": [0, 15] }] }),
    );
    s["emitters"][0]["rate"] = json!(20);
    s["emitters"][0]["max_per_step"] = json!(1);
    let mut found = Vec::new();
    for gpu in [false, true] {
        let Some(mut run) = Run::new(r, json!({ "s": s }), layers(), gpu) else { return };
        run.view.set_particle_trails(r, wprs_scene::Tier::T3);
        (0..24).for_each(|_| drop(run.frame(r, 1.0 / 30.0)));
        let slots = r.particles(&run.view).unwrap().remove(0).1.remove(0);
        found.push((r.strands(&run.view).unwrap().remove(0), slots));
        assert!(alive(r, &run).len() > 10);
    }
    let [((cpu_rope, cpu_trail), cpu), ((gpu_rope, gpu_trail), gpu)] = [found.remove(0), found.remove(0)];
    assert!(cpu_rope.iter().filter(|s| s[7] > 0.5).count() > 30);
    for (q, (a, b)) in cpu_rope.iter().zip(&gpu_rope).enumerate() {
        assert!(
            a.iter().zip(b).all(|(x, y)| (x - y).abs() <= 1e-2 * (1.0 + x.abs())),
            "rope point {q}: {a:?} {b:?}"
        );
    }
    let k = cpu_trail.len() / cpu.len();
    assert_eq!((k, cpu_trail.len()), (7, gpu_trail.len()));
    let pos = |m: &[u32; 4]| [f32::from_bits(m[0]), f32::from_bits(m[1])];
    for (i, p) in cpu.iter().enumerate().filter(|(_, p)| p.age < p.life) {
        let j = gpu.iter().position(|g| g.id == p.id && g.age < g.life).unwrap();
        for (a, b) in cpu_trail[i * k..i * k + k].iter().zip(&gpu_trail[j * k..j * k + k]) {
            let (pa, pb) = (pos(a), pos(b));
            assert!((pa[0] - pb[0]).abs() < 0.05 && (pa[1] - pb[1]).abs() < 0.05, "{pa:?} {pb:?}");
        }
    }
}
