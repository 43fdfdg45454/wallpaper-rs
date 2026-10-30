use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_render::{RenderError, Renderer, SceneAssets, SceneView};
use wprs_scene::{Descriptor, Quality, Runtime, SCHEMA, Scene, Values};

pub fn entry(body: &str) -> String {
    format!("@fragment fn fs_e(in: VertexOut) -> @location(0) vec4<f32> {{ {body} }}")
}

pub fn renderer() -> Option<&'static Renderer> {
    match Renderer::shared() {
        Ok(r) => Some(r),
        Err(e) if std::env::var_os("WPRS_REQUIRE_GPU").is_none() => {
            eprintln!("skipping GPU test: {e}");
            None
        }
        Err(e) => panic!("GPU required but unavailable: {e}"),
    }
}

pub fn scene(effects: &[(&str, String)]) -> Scene {
    let uses: Vec<Value> = effects.iter().map(|(id, _)| json!({ "effect": id })).collect();
    let files: serde_json::Map<String, Value> = effects
        .iter()
        .map(|(id, _)| {
            (
                id.to_string(),
                json!({ "descriptor": format!("fx/{id}.json"), "wgsl": format!("fx/{id}.wgsl") }),
            )
        })
        .collect();
    let layer = json!({ "id": "l", "type": "solid", "color": "#ff0000", "size": [64, 32], "effects": uses });
    let text =
        json!({ "schema": SCHEMA, "canvas": { "size": [64, 32] }, "layers": [layer], "effects": files });
    let mut scene = Scene::from_json(&text.to_string()).unwrap();
    let read = |path: &str, _: usize| {
        let (id, ext) = path.trim_start_matches("fx/").split_once('.').unwrap_or_default();
        let (_, wgsl) = effects.iter().find(|(i, _)| *i == id).ok_or("missing")?;
        let json = json!({ "id": id, "cost": {}, "passes": [{ "entry": "fs_e" }] }).to_string();
        Ok::<_, &str>(if ext == "json" { json } else { wgsl.clone() }.into_bytes())
    };
    scene.adopt(&BTreeMap::new(), read).unwrap();
    scene
}

pub fn unchecked(scene: &mut Scene, id: &str, wgsl: &str) -> u64 {
    let d = scene.loaded.get_mut(id).expect("a package effect");
    let key = (d.key ^ wgsl.bytes().fold(0u64, |h, b| h.rotate_left(5) ^ u64::from(b))) | 1;
    *d = Arc::new(Descriptor { wgsl: wgsl.into(), key, ..(**d).clone() });
    key
}

pub struct Shown {
    pub view: SceneView,
    pub runtime: Runtime,
}

pub fn show(r: &Renderer, scene: Scene) -> Result<Shown, RenderError> {
    let view = r.scene_view(&scene, &SceneAssets::default(), &Values::new())?;
    Ok(Shown { view, runtime: Runtime::new(Arc::new(scene), Values::new()) })
}

pub fn frame(r: &Renderer, shown: &mut Shown) -> Result<(), RenderError> {
    let frame = shown.runtime.frame(shown.view.view((64, 32)), Quality::Q3);
    r.render_scene(&mut shown.view, &shown.runtime, &frame, (64, 32), Quality::Q3, &[]).map(drop)
}
