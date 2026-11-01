use super::*;
use std::time::{Duration, Instant};
use wprs_scene::{Tier, over};

const BIG: [f32; 4] = [0.0, 0.0, 3840.0, 2160.0];
const BACKGROUND: [u8; 3] = [0x33, 0x66, 0x99];

pub(super) fn packaged(json: &Value, wgsl: &str, layers: Vec<Value>) -> Option<Run> {
    let r = crate::renderer()?;
    let id = json["id"].as_str().unwrap_or_default();
    let mut effects = Map::new();
    effects.insert(id.into(), json!({ "descriptor": format!("fx/{id}.json"), "wgsl": format!("fx/{id}.wgsl") }));
    let list: Vec<Value> =
        std::iter::once(json!({ "id": "bg", "type": "solid", "color": "#336699" })).chain(layers).collect();
    let canvas = json!({ "size": [3840, 2160] });
    let text = json!({ "schema": SCHEMA, "canvas": canvas, "layers": list, "effects": effects });
    let mut scene = Scene::from_json(&text.to_string()).unwrap();
    let descriptor = json.to_string();
    let files = |path: &str, _: usize| {
        Ok::<_, &str>(if path.ends_with(".json") { descriptor.clone() } else { wgsl.to_string() }.into_bytes())
    };
    scene.adopt(&BTreeMap::new(), files).unwrap_or_else(|e| panic!("{e}"));
    let view = r.scene_view(&scene, &SceneAssets::default(), &Values::new()).unwrap();
    Some(Run { r, runtime: Runtime::new(Arc::new(scene), Values::new()), view, quality: Quality::Q3 })
}

fn adjustments(layers: usize, uses: usize, effect: &str) -> Vec<Value> {
    let list: Vec<Value> = (0..uses).map(|_| json!({ "effect": effect })).collect();
    let layer = |i| json!({ "id": format!("adj{i}"), "type": "adjustment", "effects": list });
    (0..layers).map(layer).collect()
}

fn heavy(statements: usize) -> String {
    format!(
        "@fragment fn fs_mx(in: VertexOut) -> @location(0) vec4<f32> {{
            let v = tap(in.uv); var m = mat4x4<f32>(v, v.yxzw, v.zwxy, v.wzyx); let n = m * 0.2;
            for (var j = 0u; j < 1024u; j++) {{ {} }}
            return v + m[0] * 1e-30; }}",
        "m = m * n;".repeat(statements)
    )
}

fn drawn_fast(run: &mut Run) {
    let started = Instant::now();
    run.shot_at((3840, 2160)).check(&[((32, 16), BACKGROUND)]);
    assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
    assert!(!run.r.lost());
}

#[test]
fn a_heavy_package_on_a_tiny_adjustment_layer_costs_the_whole_target_and_is_left_out() {
    let pass = |p: usize| match p {
        15 => json!({ "entry": "fs_mx", "inputs": ["target:a"] }),
        0 => json!({ "entry": "fs_mx", "target": "a" }),
        p => {
            let (target, input) = [("a", "target:b"), ("b", "target:a")][p % 2];
            json!({ "entry": "fs_mx", "target": target, "inputs": [input] })
        }
    };
    let targets = json!({ "a": { "scale": 2 }, "b": { "scale": 2 } });
    let json = json!({ "id": "mx", "cost": {}, "targets": targets, "passes": (0..16).map(pass).collect::<Vec<_>>() });
    let statements = (1..=10).rev().find(|k| Descriptor::packaged("mx", &json.to_string(), &heavy(*k)).is_ok());
    let wgsl = heavy(statements.expect("a heavy effect within the limits"));
    let Some(mut run) = packaged(&json, &wgsl, adjustments(1, 1, "mx")) else { return };
    drawn_fast(&mut run);
    let limit = run.r.tier().ceiling();
    let why = format!("the effect mx on adj0 was left out: the effects of the scene would do more than {limit}");
    assert!(run.view.left_out().iter().any(|l| l.starts_with(&why)), "{:?}", run.view.left_out());
    assert!(run.runtime.scene.cost(Quality::Q3) > Tier::T3.budget() * 100.0);
}

#[test]
fn many_adjustment_layers_of_effects_stop_at_the_scene_budget() {
    let json = json!({ "id": "lt", "cost": { "taps": 16 }, "passes": [{ "entry": "fs_lt" }] });
    let wgsl = "@fragment fn fs_lt(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv); }";
    let Some(mut run) = packaged(&json, wgsl, adjustments(250, 16, "lt")) else { return };
    drawn_fast(&mut run);
    let (tier, spends) = (run.r.tier(), run.runtime.scene.spends());
    let cut = over(&spends, |_| false, tier, BIG, BIG[2] * BIG[3]);
    let kept = spends.len() - cut.len();
    assert!(!cut.is_empty() && cut[0].0 == kept, "{kept}");
    assert!(kept as f32 * spends[0].work(BIG, BIG[2] * BIG[3]) <= tier.ceiling());
    let left = run.view.left_out();
    assert!(left.len() == 250 - kept / 16 && left.iter().all(|l| l.contains("units of work a frame")));
}

#[test]
fn stateful_uses_past_the_cap_or_the_state_budget_are_left_out() {
    let json = json!({ "id": "ss", "cost": {}, "state": { "scale": 1, "settle": 1 },
        "passes": [{ "entry": "fs_step", "inputs": ["state"], "output": "state" },
                   { "entry": "fs_show", "inputs": ["input", "state"] }] });
    let wgsl = "@fragment fn fs_step(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv) * 0.5; }
        @fragment fn fs_show(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv); }";
    let Some(mut run) = packaged(&json, wgsl, adjustments(16, 16, "ss")) else { return };
    let capped = "the effect ss on adj1 was left out: the scene has more than 16 effects that keep a state";
    assert!(run.view.left_out().iter().any(|l| l == capped), "{:?}", run.view.left_out());
    run.shot_at((3840, 2160));
    let held = "the effect ss on adj0 was left out: its state textures would take the scene past 512 MiB";
    assert!(run.view.left_out().iter().any(|l| l == held), "{:?}", run.view.left_out());
    let bytes = run.view.state_bytes();
    assert!(bytes > 0 && bytes <= 512 << 20, "{bytes}");
    assert_eq!(run.view.left_out().len(), 16);
    drawn_fast(&mut run);
}
