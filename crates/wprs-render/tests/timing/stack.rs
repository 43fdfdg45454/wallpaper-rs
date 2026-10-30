use super::hostile::{Shown, renderer, show};
use super::{book, one};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::Duration;
use wprs_render::set_gpu_budget;
use wprs_scene::{Quality, SCHEMA, Scene};

const GLOW: &str = "
@fragment fn fs_down(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv); }
@fragment fn fs_blur(in: VertexOut) -> @location(0) vec4<f32> { return sample(aux0, in.uv) * 0.9; }
@fragment fn fs_show(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv) + sample(aux0, in.uv) * 0.5; }";

fn glow(layers: Value) -> Scene {
    let json = json!({ "id": "halo", "cost": { "taps": 2, "passes": 4, "scale": 0.5 },
        "targets": { "a": { "scale": 0.5 }, "b": { "scale": 0.5 } },
        "passes": [{ "entry": "fs_down", "target": "a" },
                   { "entry": "fs_blur", "target": "b", "inputs": ["previous", "target:a"] },
                   { "entry": "fs_blur", "target": "a", "inputs": ["previous", "target:b"] },
                   { "entry": "fs_show", "inputs": ["input", "target:a"] }] });
    let files = json!({ "halo": { "descriptor": "fx/halo.json", "wgsl": "fx/halo.wgsl" } });
    let text =
        json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] }, "layers": layers, "effects": files });
    let mut scene = Scene::from_json(&text.to_string()).unwrap();
    let read = |path: &str, _: usize| {
        Ok::<_, &str>(if path.ends_with(".json") { json.to_string() } else { GLOW.to_string() }.into_bytes())
    };
    scene.adopt(&BTreeMap::new(), read).unwrap();
    scene
}

fn frames(r: &wprs_render::Renderer, shown: &mut Shown, n: usize) {
    for _ in 0..n {
        shown.runtime.advance(1.0 / 60.0);
        let frame = shown.runtime.frame(shown.view.view((64, 32)), Quality::Q3);
        r.render_scene(&mut shown.view, &shown.runtime, &frame, (64, 32), Quality::Q3, &[]).unwrap();
        r.finish().unwrap();
    }
}

#[test]
fn a_package_stack_spends_no_pass_on_clearing_its_targets_or_copying_the_backdrop() {
    let _one = one();
    let Some(r) = renderer() else { return };
    book("stack");
    let spin = json!({ "keys": [[0, 0], [4, 360]] });
    let card = json!({ "id": "card", "type": "solid", "color": "#4060ff", "size": [20, 10],
        "transform": { "position": [32, 16], "rotation": spin } });
    let post = json!({ "id": "post", "type": "adjustment", "effects": [{ "effect": "halo" }] });
    let mut shown = show(r, glow(json!([card.clone(), post]))).unwrap();
    frames(r, &mut shown, 3);
    assert!(shown.view.left_out().is_empty(), "{:?}", shown.view.left_out());
    assert_eq!(
        shown.view.stats().passes,
        6,
        "one pass for the card, four for the effect and one to put it back"
    );
    let four = json!({ "id": "post", "type": "adjustment", "effects": vec![json!({ "effect": "halo" }); 4] });
    let mut shown = show(r, glow(json!([card, four]))).unwrap();
    frames(r, &mut shown, 3);
    let stats = shown.view.stats();
    assert_eq!((stats.passes, stats.textures), (18, 4), "four effects in a row: {stats:?}");
}

#[test]
fn a_frame_with_package_effects_keeps_the_hang_limit_for_the_waits_after_it() {
    let _one = one();
    let Some(r) = renderer() else { return };
    book("hang");
    let spin = json!({ "keys": [[0, 0], [4, 360]] });
    let card = json!({ "id": "card", "type": "solid", "color": "#4060ff", "size": [20, 10],
        "transform": { "position": [32, 16], "rotation": spin } });
    let post = json!({ "id": "post", "type": "adjustment", "effects": [{ "effect": "halo" }] });
    let mut shown = show(r, glow(json!([card, post]))).unwrap();
    frames(r, &mut shown, 1);
    shown.runtime.advance(1.0 / 60.0);
    let frame = shown.runtime.frame(shown.view.view((64, 32)), Quality::Q3);
    r.render_scene(&mut shown.view, &shown.runtime, &frame, (64, 32), Quality::Q3, &[]).unwrap();
    assert_eq!(r.wait_limit(), Duration::from_secs(10), "software rendering {}", r.software());
    r.finish().unwrap();
    assert!(r.wait_limit() <= r.gpu_timeout());
}

#[test]
fn built_in_effects_are_timed_but_never_left_out() {
    let _one = one();
    let Some(r) = renderer() else { return };
    book("built-in");
    let spin = json!({ "keys": [[0, 0], [4, 360]] });
    let card = json!({ "id": "card", "type": "solid", "color": "#4060ff", "size": [20, 10],
        "transform": { "position": [32, 16], "rotation": spin },
        "effects": [{ "effect": "blur", "params": { "radius": 8 } }, { "effect": "bloom" }] });
    let mut shown = show(r, glow(json!([card]))).unwrap();
    set_gpu_budget(Some(Duration::from_micros(1)));
    frames(r, &mut shown, 8);
    set_gpu_budget(None);
    assert!(shown.view.left_out().is_empty(), "{:?}", shown.view.left_out());
    assert!(!r.timestamps() || shown.view.stats().gpu_us > 0, "{:?}", shown.view.stats());
}
