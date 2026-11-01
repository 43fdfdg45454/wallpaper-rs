use super::fields::{agree, both, system};
use super::*;

fn layers() -> Value {
    json!([{ "id": "p", "type": "particles", "system": "s" }])
}

fn kid(more: Value) -> Value {
    let ring = json!({ "circle": { "center": [0, 0], "radius": 3 } });
    let mut s = system(ring, json!({ "life": [3, 3], "speed": [5, 10], "angle": [0, 360] }), more);
    (s["capacity"], s["emitters"][0]["rate"], s["emitters"][0]["burst"]) =
        (json!(512), json!(20), json!({ "count": 2 }));
    s
}

#[test]
fn children_of_parents_in_depth_agree_on_both_paths() {
    let Some(r) = renderer() else { return };
    let init = json!({ "life": [0.2, 0.5], "speed": [10, 30], "angle": [0, 360], "depth": [0, 300], "depth_speed": [-40, 40] });
    let children = json!([{ "system": "flat", "on": "death", "inherit": ["size", "velocity"] },
        { "system": "deep", "on": "follow" }, { "system": "near", "on": "spawn" }]);
    let parent = system(
        json!({ "box": [[40, 20], [120, 60]] }),
        init,
        json!({ "capacity": 64, "perspective": { "focal": 200 }, "children": children }),
    );
    let deep = json!({ "perspective": { "focal": 200 } });
    let mut near = kid(deep.clone());
    near["init"]["depth"] = json!([5, 10]);
    let systems = json!({ "s": parent, "flat": kid(json!({})), "deep": kid(deep), "near": near });
    let Some(found) = both(r, &systems, &layers(), 24) else { return };
    agree("children in depth", &found, 0.05);
    let [cpu, _] = &found;
    assert!(cpu.iter().any(|p| p.depth[0] > 5.0), "deep children took the depth of their parents");
}

#[test]
fn boids_born_at_one_point_agree_on_both_paths_and_spread() {
    let Some(r) = renderer() else { return };
    let flock = json!({ "boids": { "radius": 20, "separation": 3, "alignment": 1, "cohesion": 0.2, "max_speed": 60 } });
    let mut s = system(json!({ "point": [80, 40] }), json!({}), json!({ "forces": [flock] }));
    s["emitters"][0] = json!({ "shape": { "point": [80, 40] }, "burst": { "count": 24 } });
    let Some(first) = both(r, &json!({ "s": s.clone() }), &layers(), 1) else { return };
    agree("stacked boids", &first, 0.05);
    let Some(later) = both(r, &json!({ "s": s }), &layers(), 8) else { return };
    for (path, all) in ["cpu", "gpu"].iter().zip(&later) {
        let apart = all.iter().enumerate().flat_map(|(i, p)| {
            all[i + 1..].iter().map(move |q| (p.pos[0] - q.pos[0]).hypot(p.pos[1] - q.pos[1]))
        });
        let spread =
            all.iter().map(|p| (p.pos[0] - 80.0).hypot(p.pos[1] - 40.0)).sum::<f32>() / all.len() as f32;
        assert!(
            apart.fold(f32::MAX, f32::min) > 1e-3 && spread > 0.2,
            "{path}: the flock stayed stacked, {spread}"
        );
    }
}
