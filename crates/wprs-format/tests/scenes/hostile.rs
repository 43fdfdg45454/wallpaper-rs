use super::*;

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

#[test]
fn scripts_that_declare_nothing_must_keep_to_the_first_interface() {
    let source = tempfile::tempdir().unwrap();
    let png = image(16, 16, ImageFormat::Png);
    let write = |name: &str, data: &[u8]| std::fs::write(source.path().join(name), data).unwrap();
    let staged = |mut declared: Value, features: &[&str]| {
        declared["path"] = json!("game.js");
        let more = json!({ "scripts": [declared] });
        write(
            "scene.json",
            scene(json!([{ "id": "sky", "type": "image", "image": "sky" }]), more).as_bytes(),
        );
        write("manifest.json", manifest(features).to_json().unwrap().as_bytes());
        wprs_format::inspect(source.path())
    };
    [("preview.png", png.as_slice()), ("sky.png", &png)].iter().for_each(|(n, d)| write(n, d));
    write("game.js", b"import { shared, console } from 'wprs'; export function init() { shared.n = 1; console.log(engine); }");
    let refused = staged(json!({}), &["scripts"]).map(drop).unwrap_err().to_string();
    assert!(
        refused.contains("/scripts/0") && refused.contains("console, its init export, shared"),
        "{refused}"
    );
    let (_, facts) =
        staged(json!({ "events": ["init"], "uses": ["shared", "console"] }), &["scripts", "scripts2"])
            .unwrap();
    assert!(facts.features.contains("scripts2"), "{:?}", facts.features);
    write(
        "game.js",
        b"import { props } from 'wprs'; export function update(f) { return props.get('a') + f.time; }",
    );
    let (_, facts) = staged(json!({}), &["scripts"]).unwrap();
    assert!(!facts.features.contains("scripts2"), "{:?}", facts.features);
}
