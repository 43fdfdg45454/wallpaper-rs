use super::hostile::{Shown, entry, renderer, show};
use super::{book, busy, lines, one};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use wprs_render::{Renderer, set_gpu_budget};
use wprs_scene::{Quality, SCHEMA, Scene};

const COLUMNS: usize = 16;

fn many(n: usize, side: f64, wgsl: String) -> Scene {
    let at = |k: usize| [(k % COLUMNS) as f64 * side + side / 2.0, (k / COLUMNS) as f64 * side + side / 2.0];
    let layer = |k| {
        json!({ "id": format!("l{k}"), "type": "solid", "color": "#ff0000", "size": [side, side],
            "transform": { "position": at(k) }, "effects": [{ "effect": "busy" }] })
    };
    let layers: Vec<Value> = (0..n).map(layer).collect();
    let size = [COLUMNS as u32 * side as u32, n.div_ceil(COLUMNS) as u32 * side as u32];
    let files = json!({ "busy": { "descriptor": "fx/busy.json", "wgsl": "fx/busy.wgsl" } });
    let text = json!({ "schema": SCHEMA, "canvas": { "size": size }, "layers": layers, "effects": files });
    let mut scene = Scene::from_json(&text.to_string()).unwrap();
    let json = json!({ "id": "busy", "cost": {}, "passes": [{ "entry": "fs_e" }] }).to_string();
    let read = |path: &str, _: usize| {
        Ok::<_, &str>(if path.ends_with(".json") { json.clone() } else { wgsl.clone() }.into_bytes())
    };
    scene.adopt(&BTreeMap::new(), read).unwrap();
    scene
}

fn timed(r: &Renderer, shown: &mut Shown, size: (u32, u32)) -> Duration {
    (0..3)
        .map(|_| {
            r.finish().unwrap();
            shown.runtime.advance(1.0 / 60.0);
            let frame = shown.runtime.frame(shown.view.view(size), Quality::Q3);
            let started = Instant::now();
            r.render_scene(&mut shown.view, &shown.runtime, &frame, size, Quality::Q3, &[]).unwrap();
            r.finish().unwrap();
            started.elapsed()
        })
        .min()
        .unwrap_or_default()
}

#[test]
fn a_frame_with_more_package_spans_than_timer_slots_is_judged_whole() {
    let _one = one();
    let Some(r) = renderer() else { return };
    let path = book("spans");
    let (n, side) = (128usize, if r.software() { 32.0 } else { 240.0 });
    let size = ((COLUMNS as f64 * side) as u32, (n.div_ceil(COLUMNS) as f64 * side) as u32);
    set_gpu_budget(Some(Duration::from_secs(60)));
    let light = timed(
        r,
        &mut show(r, many(n, side, entry("return tap(in.uv) * (1.0 + frame.time * 1e-9);"))).unwrap(),
        size,
    );
    let scene = many(n, side, busy(4));
    let key = scene.loaded["busy"].key;
    let mut shown = show(r, scene).unwrap();
    let spent = timed(r, &mut shown, size).saturating_sub(light);
    assert!(shown.view.left_out().is_empty(), "{:?}", shown.view.left_out());
    set_gpu_budget(Some(spent / 2));
    let started = Instant::now();
    while shown.view.left_out().is_empty() && started.elapsed() < Duration::from_secs(5) {
        timed(r, &mut shown, size);
    }
    set_gpu_budget(None);
    let left = shown.view.left_out();
    assert!(left.len() == 1 && left[0].contains("too slow for this graphics card"), "{spent:?} {left:?}");
    assert_eq!(lines(&path, key, "1"), 0, "{}", std::fs::read_to_string(&path).unwrap_or_default());
    r.finish().unwrap();
}
