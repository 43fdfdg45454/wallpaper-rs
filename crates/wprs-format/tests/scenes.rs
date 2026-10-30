mod common;

use common::{archive, image, manifest as v1, v2};
use image::ImageFormat;
use serde_json::{Value, json};
use std::io::Cursor;
use wprs_format::{Error, Kind, Limits, Manifest, Package, Platform, features, read};

const RIPPLE: &str = "@fragment\nfn fs_ripple(in: VertexOut) -> @location(0) vec4<f32> {\n    \
    return tap(in.uv) * (0.5 + 0.5 * sin(frame.time));\n}\n";

fn scene(layers: Value, more: Value) -> String {
    let mut scene = json!({ "schema": wprs_scene::SCHEMA, "canvas": { "size": [64, 32] },
        "assets": { "sky": { "path": "sky.png" } }, "layers": layers });
    scene.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
    scene.to_string()
}

fn manifest(features: &[&str]) -> Manifest {
    let requires = json!({ "features": features });
    let mut value = serde_json::to_value(v2(Kind::Scene)).unwrap();
    value["requires"] = requires;
    serde_json::from_value(value).unwrap()
}

fn load(manifest: &Manifest, files: &[(&str, &[u8])]) -> Result<Package<Cursor<Vec<u8>>>, Error> {
    let json = manifest.to_json().unwrap();
    let png = image(16, 16, ImageFormat::Png);
    let base: [(&str, &[u8]); 3] =
        [("manifest.json", json.as_bytes()), ("preview.png", &png), ("sky.png", &png)];
    read(Cursor::new(archive(&[&base[..], files].concat())), &Limits::default())
}

#[test]
fn scenes_name_their_newer_features_and_engines_refuse_what_they_cannot_draw() {
    let layers = json!([
        { "id": "sky", "type": "image", "image": "sky", "blend": "linear_burn", "transform": { "tilt": [10, 0] } },
        { "id": "lamp", "type": "light", "light": "point", "intensity": 2 },
        { "id": "rain", "type": "sound", "sound": "rain", "stream": true }
    ]);
    let text =
        scene(layers, json!({ "assets": { "sky": { "path": "sky.png" }, "rain": { "path": "rain.wav" } } }));
    let wav = [&b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x40\x1f\0\0\x80\x3e\0\0\x02\0\x10\0data\x80\x3e\0\0"[..], &[0; 16_000]].concat();
    let files: [(&str, &[u8]); 2] = [("scene.json", text.as_bytes()), ("rain.wav", &wav)];
    let all = ["blend_modes2", "lights", "sound", "sound2", "tilt"];
    let package = load(&manifest(&all), &files).unwrap();
    assert_eq!(package.facts.features.iter().collect::<Vec<_>>(), all);
    let desktop = features::playable(&package.manifest, &package.facts, Platform::Desktop).unwrap_err();
    assert_eq!(
        desktop.to_string(),
        "this package needs a newer version of wallpaper-rs: it uses the features lights (lights its layers) and \
         tilt (tilts layers in depth)"
    );
    let phone =
        features::playable(&package.manifest, &package.facts, Platform::Android).unwrap_err().to_string();
    assert!(phone.contains("tilt") && phone.contains("lights") && !phone.contains("blend_modes2"), "{phone}");
    assert_eq!(features::lacks("android", &package.facts.features), ["sound", "sound2"]);
    assert!(package.facts.facets.iter().all(|f| f != "desktop" && f != "android"));
    let unlisted = load(&manifest(&["sound", "sound2"]), &files).map(drop).unwrap_err().to_string();
    assert!(
        unlisted.contains("add \"blend_modes2\" (mixes layers with the newer blend modes)"),
        "{unlisted}"
    );
    let mut older = v1(Kind::Scene);
    older.preview = Some("preview.png".into());
    let package = load(&older, &files).unwrap();
    assert!(features::playable(&package.manifest, &package.facts, Platform::Desktop).is_err());
    let plain = scene(json!([{ "id": "sky", "type": "image", "image": "sky", "parallax": 0.2 }]), json!({}));
    let package = load(&manifest(&["inputs.pointer"]), &[("scene.json", plain.as_bytes())]);
    assert!(package.is_err_and(|e| e.to_string().contains("inputs")));
}

#[test]
fn package_effects_are_read_compiled_and_checked() {
    let effects = json!({ "effects": { "ripple": { "descriptor": "effects/ripple.json", "wgsl": "effects/ripple.wgsl" } } });
    let text = scene(
        json!([{ "id": "sky", "type": "image", "image": "sky", "effects": [{ "effect": "ripple" }] }]),
        effects,
    );
    let descriptor =
        json!({ "id": "ripple", "passes": [{ "entry": "fs_ripple" }], "cost": { "taps": 1 } }).to_string();
    let files = |wgsl: &'static str| -> Vec<(&'static str, Vec<u8>)> {
        vec![
            ("effects/ripple.json", descriptor.clone().into_bytes()),
            ("effects/ripple.wgsl", wgsl.as_bytes().to_vec()),
        ]
    };
    let listed = manifest(&["custom_effects"]);
    let good = files(RIPPLE);
    let good: Vec<(&str, &[u8])> = good.iter().map(|(p, d)| (*p, d.as_slice())).collect();
    let mut package = load(&listed, &[&[("scene.json", text.as_bytes())][..], &good].concat()).unwrap();
    assert_eq!(package.facts.features.iter().collect::<Vec<_>>(), ["custom_effects"]);
    assert_eq!(package.scene().unwrap().effect("ripple").unwrap().uses, [wprs_scene::Use::Time]);
    let bad = files("fn fs_ripple(");
    let bad: Vec<(&str, &[u8])> = bad.iter().map(|(p, d)| (*p, d.as_slice())).collect();
    let fault =
        load(&listed, &[&[("scene.json", text.as_bytes())][..], &bad].concat()).map(drop).unwrap_err();
    assert!(fault.to_string().contains("/effects/ripple"), "{fault}");
    let missing = load(&listed, &[("scene.json", text.as_bytes())]).map(drop).unwrap_err();
    assert!(matches!(missing, Error::Missing(path) if path.starts_with("effects/ripple")));
}

#[test]
fn long_sounds_need_streaming_and_font_collections_are_allowed() {
    let frame = [&[0xffu8, 0xfb, 0x90, 0x00][..], &[0; 413]].concat();
    let mut first = frame.clone();
    first[36..44].copy_from_slice(b"Xing\0\0\0\x01");
    first[44..48].copy_from_slice(&4000u32.to_be_bytes());
    let song = [first, frame].concat();
    let layer = |stream: bool| json!([{ "id": "song", "type": "sound", "sound": "song", "stream": stream }]);
    let assets = json!({ "assets": { "sky": { "path": "sky.png" }, "song": { "path": "song.mp3" },
        "serif": { "path": "serif.ttc", "face": 1 } } });
    let streamed = scene(layer(true), assets.clone());
    let wanted = ["fonts2", "sound", "sound2"];
    let files = |text: &String| -> Vec<(&'static str, Vec<u8>)> {
        vec![
            ("scene.json", text.clone().into_bytes()),
            ("song.mp3", song.clone()),
            ("serif.ttc", b"ttcf".to_vec()),
        ]
    };
    let ok = files(&streamed);
    let ok: Vec<(&str, &[u8])> = ok.iter().map(|(p, d)| (*p, d.as_slice())).collect();
    let package = load(&manifest(&wanted), &ok).unwrap();
    assert_eq!(package.facts.features.iter().collect::<Vec<_>>(), wanted);
    let held = files(&scene(layer(false), assets));
    let held: Vec<(&str, &[u8])> = held.iter().map(|(p, d)| (*p, d.as_slice())).collect();
    let fault = load(&manifest(&wanted), &held).map(drop).unwrap_err().to_string();
    assert!(fault.contains("song.mp3") && fault.contains("lasts 104.5 s, at most 60 s"), "{fault}");
}

#[test]
fn streamed_sounds_are_measured_before_they_are_read() {
    let dir = tempfile::tempdir().unwrap();
    let names = ["a", "b", "c"];
    let layers: Vec<Value> =
        names.iter().map(|n| json!({ "id": n, "type": "sound", "sound": n, "stream": true })).collect();
    let mut assets = json!({ "sky": { "path": "sky.png" } });
    names.iter().for_each(|n| assets[n] = json!({ "path": format!("{n}.ogg") }));
    let text = scene(json!(layers), json!({ "assets": assets }));
    let png = image(16, 16, ImageFormat::Png);
    let json = manifest(&["sound", "sound2"]).to_json().unwrap();
    let files: [(&str, &[u8]); 4] = [
        ("manifest.json", json.as_bytes()),
        ("scene.json", text.as_bytes()),
        ("preview.png", &png),
        ("sky.png", &png),
    ];
    files.iter().for_each(|(name, data)| std::fs::write(dir.path().join(name), data).unwrap());
    for n in names {
        std::fs::File::create(dir.path().join(format!("{n}.ogg"))).unwrap().set_len(30 << 20).unwrap();
    }
    let fault = wprs_format::inspect(dir.path()).map(drop).unwrap_err().to_string();
    assert!(fault.contains("size of the streamed sounds"), "{fault}");
}

#[test]
fn hostile_package_shaders_are_refused_without_crashing_the_reader() {
    let effects = json!({ "effects": { "ripple": { "descriptor": "effects/ripple.json", "wgsl": "effects/ripple.wgsl" } } });
    let text = scene(
        json!([{ "id": "sky", "type": "image", "image": "sky", "effects": [{ "effect": "ripple" }] }]),
        effects,
    );
    let descriptor =
        json!({ "id": "ripple", "passes": [{ "entry": "fs_ripple" }], "cost": { "taps": 1 } }).to_string();
    let chain = format!("let a = in.uv.x; let b = a{}; return vec4<f32>(b);", "+a".repeat(20000));
    let array =
        "var a: array<vec4<f32>, 262144>; let i = u32(in.position.x); a[i] = tap(in.uv); return a[i / 2u];";
    let looped = format!(
        "var m = mat4x4<f32>(tap(in.uv), tap(in.uv), tap(in.uv), tap(in.uv)); let n = m * 0.999;
         for (var i = 0u; i < 6u; i++) {{ for (var j = 0u; j < 1024u; j++) {{ {} }} }} return m[0];",
        "m = m * n;".repeat(400)
    );
    let cases =
        [(chain, "operations deep"), (array.into(), "KiB in variables"), (looped, "operations a pixel")];
    for (body, problem) in cases {
        let wgsl = format!("@fragment fn fs_ripple(in: VertexOut) -> @location(0) vec4<f32> {{ {body} }}");
        let (text, descriptor) = (text.clone(), descriptor.clone());
        let read = std::thread::Builder::new().stack_size(2 << 20).spawn(move || {
            let files: Vec<(&str, &[u8])> = vec![
                ("scene.json", text.as_bytes()),
                ("effects/ripple.json", descriptor.as_bytes()),
                ("effects/ripple.wgsl", wgsl.as_bytes()),
            ];
            load(&manifest(&["custom_effects"]), &files).and_then(|mut p| p.scene()).map(drop)
        });
        let fault = read.unwrap().join().expect("the reader returns").unwrap_err().to_string();
        assert!(fault.contains(problem), "{fault}");
    }
}
