use super::*;

#[test]
fn meshes_are_read_from_the_package_and_refused_until_they_are_drawn() {
    let layers = json!([{ "id": "p", "type": "mesh", "mesh": "fox", "ik": [{ "chain": ["arm", "hand"], "target": [10, 10] }] }]);
    let assets = json!({ "assets": { "sky": { "path": "sky.png" }, "fox": { "path": "fox.glb" } } });
    let text = scene(layers, assets.clone());
    let package = load(&manifest(&["ik", "mesh"], json!({})), &text, &[("fox.glb", &fox())]).unwrap();
    assert_eq!(package.facts.features.iter().collect::<Vec<_>>(), ["ik", "mesh"]);
    for platform in [Platform::Desktop, Platform::Android] {
        let refused =
            features::playable(&package.manifest, &package.facts, platform).unwrap_err().to_string();
        assert!(
            refused.contains(
                "ik (reaches for targets with chains of bones) and mesh (bends pictures with a skeleton)"
            ),
            "{refused}"
        );
    }
    let wrong = json!([{ "id": "p", "type": "mesh", "mesh": "fox", "ik": [{ "chain": ["arm", "tail"], "target": [1, 1] }] }]);
    let fault = load(&manifest(&["ik", "mesh"], json!({})), &scene(wrong, assets), &[("fox.glb", &fox())]);
    assert!(fault.map(drop).unwrap_err().to_string().contains("/layers/0/ik/0/chain/1 names \"tail\""));
    let broken = load(&manifest(&["ik", "mesh"], json!({})), &text, &[("fox.glb", b"glTF junk")]);
    assert!(
        broken
            .map(drop)
            .unwrap_err()
            .to_string()
            .contains("/assets/fox: fox.glb is not a binary glTF 2.0 file")
    );
}

#[test]
fn declared_script_parts_and_media_become_inputs() {
    let more = json!({ "scripts": [{ "path": "game.js", "uses": ["audio", "timers"] }],
        "timelines": [{ "id": "song", "duration": 1, "trigger": "media:track" }] });
    let text = scene(json!([{ "id": "sky", "type": "image", "image": "sky" }]), more);
    let files: [(&str, &[u8]); 1] = [("game.js", b"export function update() {}")];
    let wanted = ["inputs.audio", "inputs.media", "media_info", "scripts", "scripts2", "timeline_events"];
    let fault =
        load(&manifest(&wanted, json!({ "inputs": ["audio"] })), &text, &files).map(drop).unwrap_err();
    assert!(
        fault.to_string().contains("the scene uses media at /timelines/0 of scene.json; add \"media\""),
        "{fault}"
    );
    let es = wprs_core::message::tr_in(&["es".into()]);
    assert_eq!(
        fault.message().told(&es),
        "inputs no válido: la escena usa media en /timelines/0 de scene.json; añade \"media\""
    );
    let wanted = ["inputs.audio", "inputs.media", "media_info", "scripts", "scripts2"];
    let package = load(&manifest(&wanted, json!({ "inputs": ["audio", "media"] })), &text, &files).unwrap();
    assert_eq!(package.facts.inputs.iter().collect::<Vec<_>>(), [&Input::Audio, &Input::Media]);
    let declared = package.manifest.declared().unwrap();
    assert!(declared.audio && declared.media && !declared.cursor);
    let phone =
        features::playable(&package.manifest, &package.facts, Platform::Android).unwrap_err().to_string();
    assert!(phone.contains("media_info") && !phone.contains("scripts2"), "{phone}");
    assert!(features::lacks("android", &package.facts.features).is_empty(), "the phone runs scripts");
}

#[test]
fn rgb_output_and_spanned_scenes_need_their_manifest_fields() {
    let lights =
        scene(json!([{ "id": "rgb", "type": "adjustment", "size": [32, 8], "rgb": true }]), json!({}));
    let fault =
        load(&manifest(&["capture", "rgb"], json!({})), &lights, &[]).map(drop).unwrap_err().to_string();
    assert!(
        fault.contains("permissions: the scene sends its colors to RGB lighting; add \"rgb\": true"),
        "{fault}"
    );
    let granted = json!({ "permissions": { "rgb": true } });
    let package = load(&manifest(&["capture", "rgb"], granted), &lights, &[]).unwrap();
    assert_eq!(features::lacks("android", &package.facts.features), ["rgb"]);
    assert!(features::playable(&package.manifest, &package.facts, Platform::Desktop).is_err());
    let networked = json!({ "permissions": { "network": { "allow": ["https://api.example.org"] } } });
    let fault =
        load(&manifest(&["capture", "rgb"], networked), &lights, &[]).map(drop).unwrap_err().to_string();
    assert!(fault.contains("permissions.network"), "{fault}");
    let plain = scene(json!([{ "id": "sky", "type": "image", "image": "sky" }]), json!({}));
    let spread = json!({ "display": { "span": "all-outputs" } });
    let package = load(&manifest(&["display2", "spanned_scenes"], spread), &plain, &[]).unwrap();
    let desktop =
        features::playable(&package.manifest, &package.facts, Platform::Desktop).unwrap_err().to_string();
    assert!(desktop.contains("spanned_scenes (spreads its scene over every screen)"), "{desktop}");
    assert!(features::playable(&package.manifest, &package.facts, Platform::Android).is_ok());
    let flipped = json!({ "display": { "flip": "horizontal" } });
    let fault =
        load(&manifest(&["display2", "flip"], flipped), &plain, &[]).map(drop).unwrap_err().to_string();
    assert!(fault.contains("fit, background and flip apply to pictures and videos only"), "{fault}");
}

#[test]
fn missing_inputs_point_at_the_script_or_asset_that_needs_them() {
    let more = json!({ "scripts": [{ "path": "game.js", "uses": ["pointer"] }],
        "assets": { "sky": { "path": "sky.png" }, "cover": { "path": "sky.png", "media": "previous" } } });
    let text = scene(json!([{ "id": "sky", "type": "image", "image": "sky" }]), more);
    let files: [(&str, &[u8]); 1] = [("game.js", b"export function update() {}")];
    let wanted = ["inputs.media", "inputs.pointer", "media_info", "scripts"];
    let pointer =
        load(&manifest(&wanted, json!({ "inputs": ["media"] })), &text, &files).map(drop).unwrap_err();
    assert!(pointer.to_string().contains("pointer at /scripts/0 of scene.json"), "{pointer}");
    let media =
        load(&manifest(&wanted, json!({ "inputs": ["pointer"] })), &text, &files).map(drop).unwrap_err();
    assert!(media.to_string().contains("media at /assets/cover of scene.json"), "{media}");
    load(&manifest(&wanted, json!({ "inputs": ["media", "pointer"] })), &text, &files).unwrap();
}
