use super::*;

fn range(path: &str, value: f32, min: f32, max: f32) -> Issue {
    Issue::Range { path: format!("/particles/dots{path}"), value, min, max }
}

#[test]
fn steering_flocks_walls_and_strands_refuse_bad_values() {
    let cases = [
        (
            json!({ "forces": [{ "keep_distance": { "target": [10, 10], "distance": -1, "strength": 1 } }] }),
            range("/forces/0/keep_distance/distance", -1.0, 0.0, 1e5),
        ),
        (
            json!({ "forces": [{ "keep_between": { "from": "p0", "to": [5, 5], "strength": 2000 } }] }),
            range("/forces/0/keep_between/strength", 2000.0, -1e3, 1e3),
        ),
        (
            json!({ "forces": [{ "boids": { "radius": 0, "separation": 1, "alignment": 1, "cohesion": 1, "max_speed": 9 } }] }),
            range("/forces/0/boids/radius", 0.0, 1.0, 1e4),
        ),
        (
            json!({ "forces": [{ "boids": { "radius": 9, "separation": 11, "alignment": 1, "cohesion": 1, "max_speed": 9 } }] }),
            range("/forces/0/boids/separation", 11.0, 0.0, 10.0),
        ),
        (
            json!({ "capacity": 9000, "forces": [{ "boids": { "radius": 9, "separation": 1, "alignment": 1, "cohesion": 1, "max_speed": 9 } }] }),
            range("/capacity", 9000.0, 1.0, 8192.0),
        ),
        (json!({ "render": { "rope": { "subdivide": 8 } } }), range("/render/rope/subdivide", 8.0, 0.0, 7.0)),
        (json!({ "render": { "rope": { "width": -2 } } }), range("/render/rope/width", -2.0, 0.0, 1e3)),
        (
            json!({ "render": { "trail": { "length": 65, "width": 1 } } }),
            range("/render/trail/length", 65.0, 1.0, 64.0),
        ),
        (
            json!({ "render": { "trail": { "length": 30, "width": 1, "segments": 0 } } }),
            range("/render/trail/segments", 0.0, 1.0, 64.0),
        ),
        (
            json!({ "render": { "trail": { "length": 30, "width": 1, "uv_scroll": 200 } } }),
            range("/render/trail/uv_scroll", 200.0, -100.0, 100.0),
        ),
        (
            json!({ "collide": [{ "bounds": { "circle": { "center": [5, 5], "radius": 0, "inside": true } } }] }),
            range("/collide/0/bounds/circle/radius", 0.0, 1e-3, 1e5),
        ),
        (
            json!({ "collide": [{ "bounds": "canvas", "friction": 3, "stick": true }] }),
            range("/collide/0/friction", 3.0, 0.0, 1.0),
        ),
    ];
    for (bad, issue) in cases {
        assert_eq!(issues(with(sparks(), bad.clone())), [issue], "{bad}");
    }
    let flat = json!({ "collide": [{ "bounds": { "segment": { "from": [5, 5], "to": [5, 5] } } }] });
    assert_eq!(issues(with(sparks(), flat)), [Issue::Type("/particles/dots/collide/0/bounds".into())]);
    let gone = json!({ "collide": [{ "bounds": { "layer": { "id": "nowhere" } } }] });
    assert!(matches!(&issues(with(sparks(), gone))[..], [Issue::Unknown { what: "layer", .. }]));
    let remap =
        json!({ "forces": [{ "remap": { "input": "age", "in": [0, 1], "output": "glow", "out": [0, 1] } }] });
    assert!(Scene::from_json(&scene(with(sparks(), remap), true)).is_err());
}

#[test]
fn each_new_particle_field_names_its_feature() {
    let uses = [
        (
            json!({ "forces": [{ "remap": { "input": "age", "in": [0, 1], "output": "size", "out": [1, 4] } }] }),
            "particle_steering",
        ),
        (
            json!({ "forces": [{ "boids": { "radius": 9, "separation": 1, "alignment": 1, "cohesion": 1, "max_speed": 9 } }] }),
            "boids",
        ),
        (json!({ "collide": [{ "bounds": "canvas", "stick": true }] }), "particle_collisions"),
        (
            json!({ "collide": [{ "bounds": { "circle": { "center": [5, 5], "radius": 9, "inside": true } } }] }),
            "particle_collisions",
        ),
        (json!({ "render": { "trail": { "length": 8, "width": 1, "textured": true } } }), "particle_ropes"),
        (json!({ "render": { "rope": {} } }), "particle_ropes"),
    ];
    for (more, feature) in uses {
        let found = Scene::from_json(&scene(with(sparks(), more.clone()), true)).unwrap();
        assert_eq!(found.validate(&props()), [], "{more}");
        assert!(found.features().contains(feature), "{more}");
    }
    assert!(!Scene::from_json(&scene(sparks(), true)).unwrap().features().contains("particle_ropes"));
}
