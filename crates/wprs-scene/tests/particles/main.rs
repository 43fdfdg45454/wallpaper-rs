use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

mod extremes;
mod fields;
mod scheduled;
mod validated;

const VIEW: [f32; 4] = [0.0, 0.0, 200.0, 100.0];

fn props() -> BTreeMap<String, Property> {
    serde_json::from_str(include_str!("../props.json")).unwrap()
}

fn sparks() -> Json {
    json!({
        "capacity": 500, "seed": 3,
        "emitters": [{ "shape": { "box": [[0, 80], [200, 100]] }, "rate": 200 }],
        "init": { "life": [0.5, 1.5], "speed": [20, 60], "angle": [250, 290], "size": [2, 4] },
        "forces": [{ "gravity": [0, 30] }, { "turbulence": { "strength": 20, "scale": 0.05 } }, { "drag": 0.2 }],
        "collide": [{ "bounds": "canvas", "bounce": 0.5 }]
    })
}

fn with(mut base: Json, patch: Json) -> Json {
    patch.as_object().unwrap().iter().for_each(|(k, v)| base[k] = v.clone());
    base
}

fn scene(system: Json, visible: bool) -> String {
    let layer = json!({ "id": "p", "type": "particles", "system": "dots", "visible": visible });
    let assets = json!({ "dot": { "path": "dot.png", "atlas": [2, 2] } });
    json!({ "schema": SCHEMA, "canvas": { "size": [200, 100] }, "layers": [layer], "particles": { "dots": system }, "assets": assets })
        .to_string()
}

fn runtime(text: &str) -> Runtime {
    let props = props();
    Runtime::new(Arc::new(Scene::load(text, &props).unwrap()), effective(&props, &[]))
}

fn run(text: &str, frames: usize, quality: Quality) -> Cloud {
    let mut rt = runtime(text);
    let bake = Bake::new(&rt.scene, 0, Tier::T3, false, |_| None).unwrap();
    let (mut cloud, mut stamp) = (Cloud::new(bake.allocs[0]), 0);
    for _ in 0..frames {
        rt.advance(1.0 / 30.0);
        rt.frame(VIEW, quality);
        let swarm = rt.swarm(0).unwrap();
        if std::mem::replace(&mut stamp, swarm.stamp) != swarm.stamp {
            swarm.steps.iter().for_each(|s| cloud.step(&bake.apply(0, s), &bake.points));
        }
    }
    cloud
}

fn issues(system: Json) -> Vec<Issue> {
    Scene::from_json(&scene(system, true)).unwrap().validate(&props())
}
