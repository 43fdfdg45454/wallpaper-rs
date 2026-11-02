use super::*;
use wprs_scene::Wake;

fn ball(m: ForgeMaterial) -> Vec<u8> {
    Forge::default().material(m).part("ball", ForgeShape::Sphere(12), [0.0; 3], 2.0, 0).glb()
}

fn plane(m: ForgeMaterial) -> Vec<u8> {
    Forge::default().material(m).part("card", ForgeShape::Plane, [0.0; 3], 3.0, 0).glb()
}

fn shaded(model: &str, material: Value) -> Value {
    json!([{ "id": "m", "type": "model", "model": model, "materials": { "skin": material } }])
}

fn covered(px: &[u8]) -> usize {
    (0..SIDE * SIDE).filter(|i| rgb(px, i % SIDE, i / SIDE).iter().any(|c| *c > 8)).count()
}

#[test]
fn fur_grows_shells_past_the_skin_and_fewer_at_the_lowest_quality() {
    let Some(r) = renderer() else { return };
    let models = || BTreeMap::from([("ball", ball(unlit("skin", [0.9, 0.7, 0.4, 1.0])))]);
    let fur = json!({ "shader": "fur", "params": { "layers": 16, "length": 40, "density": 0.6 } });
    let bare = render(r, scene(json!({}), shaded("ball", json!({})), models()), Quality::Q3);
    let furry = render(r, scene(json!({}), shaded("ball", fur.clone()), models()), Quality::Q3);
    let low = render(r, scene(json!({}), shaded("ball", fur), models()), Quality::Q0);
    assert!(covered(&furry) > covered(&bare) + 40, "{} {}", covered(&furry), covered(&bare));
    assert!(covered(&low) > covered(&bare) && low != furry);
    assert_eq!(wprs_scene::fur_shells(16, Quality::Q0), 4);
}

#[test]
fn foliage_sways_with_the_wind_and_sleeps_without_it() {
    let Some(r) = renderer() else { return };
    let models = || BTreeMap::from([("card", plane(unlit("skin", [0.2, 0.9, 0.2, 1.0])))]);
    let leaf = json!({ "shader": "foliage", "params": { "strength": 6, "speed": 2 } });
    let windy = json!({ "canvas": { "size": [SIDE, SIDE], "background": "#000000", "wind": [400, 0] } });
    let mut calm = shoot(r, scene(json!({}), shaded("card", leaf.clone()), models()), &[]);
    let a = calm.frame(r, 0.0, Quality::Q3);
    assert_eq!(calm.frame(r, 0.3, Quality::Q3), a);
    assert_eq!(calm.runtime.frame(calm.view.view((SIDE, SIDE)), Quality::Q3).wake, Wake::Idle);
    let mut blown = shoot(r, scene(windy, shaded("card", leaf), models()), &[]);
    let (b, c) = (blown.frame(r, 0.0, Quality::Q3), blown.frame(r, 0.4, Quality::Q3));
    assert_ne!(b, c, "the top of the card moves");
    assert_eq!(blown.runtime.frame(blown.view.view((SIDE, SIDE)), Quality::Q3).wake, Wake::Frame);
    assert_eq!(rgb(&b, 32, 46), rgb(&c, 32, 46), "the foot stays");
}

#[test]
fn chroma_keys_out_its_color_and_keeps_the_rest() {
    let Some(r) = renderer() else { return };
    let models = || BTreeMap::from([("card", plane(unlit("skin", [0.0, 1.0, 0.0, 1.0])))]);
    let green =
        json!({ "shader": "chroma", "params": { "key": "#00ff00", "threshold": 0.2, "softness": 0.1 } });
    let red =
        json!({ "shader": "chroma", "params": { "key": "#ff0000", "threshold": 0.2, "softness": 0.1 } });
    let gone = render(r, scene(json!({}), shaded("card", green), models()), Quality::Q3);
    let kept = render(r, scene(json!({}), shaded("card", red), models()), Quality::Q3);
    assert!(rgb(&gone, 32, 32)[1] < 10, "{:?}", rgb(&gone, 32, 32));
    assert!(rgb(&kept, 32, 32)[1] > 200, "{:?}", rgb(&kept, 32, 32));
}

const TINT: &str = "fn material(s: Surface) -> Surface {\n    var o = s;\n    o.albedo = vec4<f32>(param(0u).x, 0.0, 1.0, 1.0);\n    return o;\n}\n";

pub fn packaged(layers: Value, files: &[(&'static str, &'static str)]) -> Scene {
    let effects: serde_json::Map<String, Value> = files
        .iter()
        .filter(|(p, _)| p.ends_with(".json"))
        .map(|(p, _)| {
            let id = p.trim_start_matches("effects/").trim_end_matches(".json");
            (id.to_string(), json!({ "descriptor": p, "wgsl": p.replace(".json", ".wgsl") }))
        })
        .collect();
    let doc = json!({ "schema": SCHEMA, "canvas": { "size": [SIDE, SIDE], "background": "#000000" },
        "assets": { "ball": { "path": "ball.glb" } }, "layers": layers, "effects": effects });
    let mut scene = Scene::from_json(&doc.to_string()).unwrap();
    let glb = ball(unlit("skin", [1.0; 4]));
    let files: BTreeMap<&str, Vec<u8>> = files.iter().map(|(p, t)| (*p, t.as_bytes().to_vec())).collect();
    let read = move |path: &str, _: usize| match path {
        "ball.glb" => Ok(glb.clone()),
        p => files.get(p).cloned().ok_or_else(|| format!("no {p}")),
    };
    scene.adopt(&BTreeMap::new(), read).unwrap_or_else(|e| panic!("{e}"));
    scene
}

#[test]
fn a_package_material_shader_shades_the_surface_with_its_params() {
    let Some(r) = renderer() else { return };
    let files = [
        (
            "effects/hue_coat.json",
            r#"{ "id": "hue_coat", "stage": "material", "cost": {}, "params": [{ "id": "red", "type": "float", "default": 0.5, "min": 0, "max": 1 }] }"#,
        ),
        ("effects/hue_coat.wgsl", TINT),
    ];
    let layers = shaded("ball", json!({ "shader": "hue_coat", "params": { "red": 1.0 } }));
    let px = render(r, packaged(layers, &files), Quality::Q3);
    let c = rgb(&px, 32, 32);
    assert!(c[0] > 200 && c[1] < 10 && c[2] > 200, "{c:?}");
}
