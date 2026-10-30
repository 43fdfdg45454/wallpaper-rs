use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_render::{Renderer, SceneAssets, SceneView};
use wprs_scene::{Quality, Runtime, SCHEMA, Scene, Values};

const SIZE: (u32, u32) = (160, 80);

fn renderer() -> Option<&'static Renderer> {
    match Renderer::shared() {
        Ok(r) => Some(r),
        Err(e) if std::env::var_os("WPRS_REQUIRE_GPU").is_none() => {
            eprintln!("skipping GPU test: {e}");
            None
        }
        Err(e) => panic!("GPU required but unavailable: {e}"),
    }
}

fn open(r: &Renderer, systems: Value, layers: Value, gpu: bool) -> Option<(Runtime, SceneView)> {
    let canvas = json!({ "size": [SIZE.0, SIZE.1], "background": "#000000" });
    let text = json!({ "schema": SCHEMA, "canvas": canvas, "layers": layers, "particles": systems });
    let scene = Scene::load(&text.to_string(), &BTreeMap::new()).unwrap_or_else(|e| panic!("{e}"));
    let mut view = r.scene_view(&scene, &SceneAssets::default(), &Values::new()).unwrap();
    view.set_gpu_particles(r, gpu).then(|| (Runtime::new(Arc::new(scene), Values::new()), view))
}

fn render(r: &Renderer, runtime: &Runtime, view: &mut SceneView) -> (u32, u32, u32) {
    let frame = runtime.frame(view.view(SIZE), Quality::Q3);
    r.render_scene(view, runtime, &frame, SIZE, Quality::Q3, &[]).unwrap();
    let s = view.stats();
    (s.damage, s.steps, s.created)
}

fn sparks(life: f32) -> Value {
    json!({ "sparks": { "capacity": 400, "seed": 3,
        "emitters": [{ "shape": { "circle": { "center": [40, 40], "radius": 8 } }, "burst": { "count": 200 } }],
        "init": { "life": [life, life], "speed": [10, 40], "size": [3, 6], "color": ["#ffd27a", "#ff7a3d"] } } })
}

#[test]
fn a_held_scene_with_particles_stays_clean_after_its_view_is_trimmed() {
    let Some(r) = renderer() else { return };
    let layers = json!([{ "id": "p", "type": "particles", "system": "sparks" }]);
    for gpu in [false, true] {
        let Some((mut runtime, mut view)) = open(r, sparks(30.0), layers.clone(), gpu) else { return };
        runtime.seek(1.0);
        assert_eq!(render(r, &runtime, &mut view).0, 100);
        for _ in 0..3 {
            view.trim();
            assert_eq!(render(r, &runtime, &mut view), (0, 0, 0), "gpu {gpu}");
            assert_eq!(view.damage(), Some(Default::default()));
        }
        runtime.advance(0.1);
        assert_eq!(render(r, &runtime, &mut view).0, 100, "a trimmed view that moves again redraws all");
    }
}

#[test]
fn particles_that_no_longer_move_leave_the_damage_to_what_does() {
    let Some(r) = renderer() else { return };
    let slide = json!({ "keys": [[0, [120, 20]], [4, [140, 60]]] });
    let dot = json!({ "id": "dot", "type": "solid", "color": "#ffffff", "size": [6, 6], "transform": { "position": slide } });
    let layers = json!([{ "id": "p", "type": "particles", "system": "sparks" }, dot]);
    for gpu in [false, true] {
        let Some((mut runtime, mut view)) = open(r, sparks(0.2), layers.clone(), gpu) else { return };
        let mut damage = Vec::new();
        for _ in 0..12 {
            runtime.advance(0.1);
            damage.push(render(r, &runtime, &mut view));
        }
        assert!(damage[..2].iter().all(|d| d.0 == 100 && d.1 > 0), "gpu {gpu}: {damage:?}");
        let idle = &damage[6..];
        assert!(
            idle.iter().all(|&(part, steps, _)| part > 0 && part < 10 && steps == 0),
            "gpu {gpu}: {damage:?}"
        );
        let region = view.damage().expect("a partial frame");
        assert!(region.x >= 110 && region.width < 30 && !region.is_empty(), "{region:?}");
    }
}
