use super::*;
use image::{Rgba, RgbaImage};

fn dot(at: [f32; 2], more: Value) -> Value {
    let mut s = json!({ "capacity": 4, "seed": 2, "emitters": [{ "shape": { "point": at }, "burst": { "count": 1 } }],
        "init": { "life": [30, 30], "speed": [0, 0], "size": [24, 24], "color": ["#ffffff", "#ffffff"] } });
    more.as_object().into_iter().flatten().for_each(|(k, v)| s[k] = v.clone());
    s
}

fn seen(r: &Renderer, system: Value, files: &[(&str, &str)], gpu: bool) -> Option<(Vec<u8>, Vec<String>)> {
    let canvas = json!({ "size": [SIZE.0, SIZE.1], "background": "#000000" });
    let layers = json!([{ "id": "p", "type": "particles", "system": "s" }]);
    let effects: serde_json::Map<String, Value> = files
        .iter()
        .filter(|(p, _)| p.ends_with(".json"))
        .map(|(p, _)| {
            let id = p.trim_start_matches("effects/").trim_end_matches(".json");
            (id.to_string(), json!({ "descriptor": p, "wgsl": p.replace(".json", ".wgsl") }))
        })
        .collect();
    let assets = json!({ "pair": { "path": "pair.png", "atlas": [1, 1] } });
    let text = json!({ "schema": SCHEMA, "canvas": canvas, "layers": layers, "particles": { "s": system },
        "assets": assets, "effects": effects });
    let files: BTreeMap<&str, &str> = files.iter().copied().collect();
    let mut scene = Scene::from_json(&text.to_string()).unwrap();
    let read = |path: &str, _: usize| files.get(path).map(|t| t.as_bytes().to_vec()).ok_or("missing");
    scene.adopt(&BTreeMap::new(), read).unwrap_or_else(|e| panic!("{e}"));
    let pair =
        RgbaImage::from_fn(2, 1, |x, _| Rgba(if x == 0 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }));
    let images = [("pair".to_string(), Pixels::new(pair))].into();
    let mut view =
        r.scene_view(&scene, &SceneAssets { images, ..Default::default() }, &Values::new()).unwrap();
    if !view.set_gpu_particles(r, gpu) {
        return None;
    }
    let mut run = Run { runtime: Runtime::new(Arc::new(scene), Values::new()), view };
    run.frame(r, 0.0);
    let out = run.frame(r, 1.0 / 30.0);
    Some((out, run.view.left_out()))
}

fn shot(r: &Renderer, system: Value, files: &[(&str, &str)], gpu: bool) -> Option<Vec<u8>> {
    seen(r, system, files, gpu).map(|(p, _)| p)
}

fn row(pixels: &[u8], y: u32) -> Vec<u8> {
    (0..SIZE.0).map(|x| at(pixels, x, y)[0]).collect()
}

#[test]
fn cutouts_harden_the_alpha_of_a_sprite_and_set_its_opacity() {
    let Some(r) = renderer() else { return };
    for gpu in [false, true] {
        let soft = shot(r, dot([80.0, 40.0], json!({})), &[], gpu);
        let hard = shot(r, dot([80.0, 40.0], json!({ "render": { "cutout": [0.45, 0.55, 0.5] } })), &[], gpu);
        let (Some(soft), Some(hard)) = (soft, hard) else { return };
        let between = |p: &[u8]| row(p, 40).iter().filter(|v| (20..100).contains(*v)).count();
        assert!(between(&hard) < between(&soft), "gpu {gpu}: {:?} {:?}", row(&soft, 40), row(&hard, 40));
        assert!(
            (170..200).contains(&at(&hard, 80, 40)[0]),
            "gpu {gpu}: half opacity {:?}",
            at(&hard, 80, 40)
        );
    }
}

#[test]
fn orientation_turns_sprites_to_the_screen_their_motion_or_a_fixed_angle() {
    let Some(r) = renderer() else { return };
    let paired = |more: Value| {
        let mut s = dot([80.0, 40.0], json!({ "render": { "texture": "pair" } }));
        more.as_object().into_iter().flatten().for_each(|(k, v)| s["render"][k] = v.clone());
        s
    };
    for gpu in [false, true] {
        let Some(plain) = shot(r, paired(json!({})), &[], gpu) else { return };
        let fixed = shot(r, paired(json!({ "orient": "fixed", "angle": 90 })), &[], gpu).unwrap();
        let mut moving = paired(json!({ "orient": "velocity" }));
        moving["init"]["velocity"] = json!([[-1, 0], [-1, 0]]);
        let moving = shot(r, moving, &[], gpu).unwrap();
        assert!(at(&plain, 74, 40)[0] > 200 && at(&plain, 86, 40)[2] > 200, "gpu {gpu}: red on the left");
        assert!(at(&fixed, 80, 34)[0] > 200 && at(&fixed, 80, 46)[2] > 200, "gpu {gpu}: red on top");
        assert!(at(&moving, 86, 40)[0] > 200 && at(&moving, 74, 40)[2] > 200, "gpu {gpu}: red behind");
    }
}

#[test]
fn perspective_draws_far_particles_smaller_and_nearer_the_middle() {
    let Some(r) = renderer() else { return };
    for gpu in [false, true] {
        let s = dot(
            [20.0, 40.0],
            json!({ "perspective": { "focal": 200 }, "init": { "life": [30, 30], "speed": [0, 0],
            "size": [24, 24], "depth": [200, 200], "color": ["#ffffff", "#ffffff"] } }),
        );
        let Some(p) = shot(r, s, &[], gpu) else { return };
        assert!(at(&p, 50, 40)[0] > 200, "gpu {gpu}: at the projected spot {:?}", row(&p, 40));
        assert!(
            at(&p, 20, 40)[0] < 10 && at(&p, 58, 40)[0] < 10,
            "gpu {gpu}: half as large {:?}",
            row(&p, 40)
        );
    }
}

const GREEN: &str = "fn particle(p: Sprite) -> Sprite {\n    var o = p;\n    o.color = vec4<f32>(0.0, p.color.a, 0.0, p.color.a);\n    o.position = p.position + vec2<f32>(20.0, 0.0);\n    return o;\n}\n";

#[test]
fn package_particle_shaders_move_and_color_each_corner() {
    let Some(r) = renderer() else { return };
    let files = [
        ("effects/green.json", r#"{ "id": "green", "stage": "particle", "cost": {} }"#),
        ("effects/green.wgsl", GREEN),
    ];
    for gpu in [false, true] {
        let s = dot([60.0, 40.0], json!({ "render": { "shader": "green" } }));
        let Some(p) = shot(r, s, &files, gpu) else { return };
        assert!(at(&p, 80, 40)[1] > 200 && at(&p, 80, 40)[0] < 10, "gpu {gpu}: {:?}", at(&p, 80, 40));
        assert!(at(&p, 56, 40) == [0, 0, 0], "gpu {gpu}: moved by the shader");
    }
}

const TEXTURED: &str = "fn particle(p: Sprite) -> Sprite {\n    var o = p;\n    o.color = textureSampleLevel(input_tex, samp, p.uv, 0.0);\n    return o;\n}\n";

#[test]
fn particle_shaders_the_engine_cannot_build_are_left_out_and_particles_still_draw() {
    let Some(r) = renderer() else { return };
    let files = [
        ("effects/odd.json", r#"{ "id": "odd", "stage": "particle", "cost": {} }"#),
        ("effects/odd.wgsl", TEXTURED),
    ];
    for gpu in [false, true] {
        let s = dot([60.0, 40.0], json!({ "render": { "shader": "odd" } }));
        let Some((p, left)) = seen(r, s, &files, gpu) else { return };
        assert!(left.iter().any(|l| l.contains("odd")), "gpu {gpu}: {left:?}");
        assert!(at(&p, 60, 40)[0] > 200, "gpu {gpu}: drawn plainly");
    }
}
