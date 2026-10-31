use super::*;

#[test]
fn origins_name_imports_and_providers_and_credits_name_their_package() {
    let provider = json!({ "kind": "provider", "source_url": "https://example.org/item/1",
        "provider": { "name": "example-photos", "item_url": "https://example.org/item/1", "credit": "Photo by Jo" } });
    let from = with(Kind::Image, json!({ "origin": provider, "license": "CC-BY-4.0" }));
    from.check().unwrap();
    from.publishable().unwrap();
    assert_eq!(named(&from), ["origin2"]);
    let closed = Manifest { license: "LicenseRef-provider-terms".into(), ..from.clone() };
    assert_eq!(field(closed.publishable()), "license");
    let nameless = json!({ "kind": "provider" });
    assert_eq!(field(with(Kind::Image, json!({ "origin": nameless })).check()), "origin.provider");
    let stray = json!({ "kind": "original", "provider": provider["provider"] });
    assert_eq!(field(with(Kind::Image, json!({ "origin": stray })).check()), "origin.provider");
    let bad = json!({ "kind": "provider", "provider": { "name": "Bad Name", "item_url": "https://x.org", "credit": "Jo" } });
    assert_eq!(field(with(Kind::Image, json!({ "origin": bad })).check()), "origin.provider");
    let imported = with(Kind::Image, json!({ "origin": { "kind": "imported" } }));
    imported.check().unwrap();
    assert_eq!(field(imported.publishable()), "origin.kind");
    let credit = |version: &str| {
        json!([{ "paths": ["image.png"], "license": "CC0-1.0", "attribution": "Jo",
        "source": { "id": "org.example.fx", "version": version } }])
    };
    let credited = with(Kind::Image, json!({ "credits": credit("1.2.0") }));
    credited.check().unwrap();
    assert_eq!(named(&credited), ["credits2"]);
    assert_eq!(field(with(Kind::Image, json!({ "credits": credit("^1") })).check()), "credits");
}

#[test]
fn display_and_playback_fields_belong_to_their_kinds() {
    let shown = |kind: Kind, display: Value| with(kind, json!({ "display": display }));
    let picture = shown(Kind::Image, json!({ "fit": "fit", "background": "#102030", "span": "all-outputs" }));
    picture.check().unwrap();
    assert_eq!(named(&picture), ["display2"]);
    assert_eq!(named(&shown(Kind::Video, json!({ "flip": "horizontal" }))), ["flip", "video_properties"]);
    assert_eq!(named(&shown(Kind::Scene, json!({ "accent": "#ff8800" }))), ["accent"]);
    let unread = ["accent".to_string(), "gpu".into()];
    let platforms = Platform::ALL.map(|p| features::lacks(p.name(), &unread));
    assert!(platforms.iter().all(|lacked| *lacked == unread), "{platforms:?}");
    let tinted = Manifest {
        properties: serde_json::from_value(json!({ "tint": { "type": "color", "default": "#ff8800" },
            "on": { "type": "bool", "default": true } }))
        .unwrap(),
        ..shown(Kind::Scene, json!({ "accent": { "prop": "tint" } }))
    };
    tinted.check().unwrap();
    let wrong = Manifest {
        display: serde_json::from_value(json!({ "accent": { "prop": "on" } })).unwrap(),
        ..tinted
    };
    assert_eq!(field(wrong.check()), "display.accent");
    assert_eq!(field(shown(Kind::Scene, json!({ "flip": "vertical" })).check()), "display");
    assert_eq!(field(shown(Kind::Image, json!({ "zoom": 2 })).check()), "display");
    assert_eq!(field(shown(Kind::Image, json!({ "background": "red" })).check()), "display");
    assert_eq!(field(shown(Kind::Scene, json!({ "accent": "#ff88" })).check()), "display");
    let play = |kind: Kind, playback: Value| with(kind, json!({ "playback": playback }));
    let video = play(Kind::Video, json!({ "max_fps": "display", "muted": true, "loop": false }));
    video.check().unwrap();
    assert_eq!(named(&video), ["playback2", "video_properties"]);
    assert_eq!(
        serde_json::to_value(&video).unwrap()["playback"],
        json!({ "max_fps": "display", "muted": true, "loop": false })
    );
    play(Kind::Web, json!({ "max_fps": 30, "volume": 0.5 })).check().unwrap();
    assert_eq!(field(play(Kind::Video, json!({ "max_fps": 0 })).check()), "playback");
    assert_eq!(field(play(Kind::Video, json!({ "max_fps": 241 })).check()), "playback");
    assert_eq!(field(play(Kind::Image, json!({ "volume": 0.5 })).check()), "playback");
    let facts = features::facts(&video, None, None);
    assert!(
        features::playable(&video, &facts, Platform::Desktop).unwrap_err().to_string().contains("playback2")
    );
}

#[test]
fn shortcuts_lists_of_folders_and_quality_values_are_checked() {
    let props = |properties: Value| Manifest {
        properties: serde_json::from_value(properties).unwrap(),
        ..v2(Kind::Scene)
    };
    let all = props(json!({
        "open": { "type": "shortcut", "label": "Open" },
        "photos": { "type": "folder", "mode": "all", "max_files": 200, "recursive": 2 },
        "speed": { "type": "slider", "min": 0, "max": 2, "step": 0.1, "default": 1, "quality": { "q1": 0.5, "q0": 0.2 } },
        "glow": { "type": "bool", "default": true, "quality": { "q0": false } }
    }));
    all.check().unwrap();
    let found = named(&all);
    for name in ["folder", "folder2", "folder_lists", "properties2", "quality_settings", "shortcut"] {
        assert!(found.contains(&name.to_string()), "{name} in {found:?}");
    }
    let open = &all.properties["open"];
    for (value, ok) in [
        (json!({ "uri": "https://example.org" }), true),
        (json!({ "app": "org.kde.dolphin" }), true),
        (json!({ "folder": "/home" }), true),
        (json!(null), true),
        (json!({ "uri": "http://example.org" }), false),
        (json!({ "file": "notes.txt" }), false),
        (json!({ "app": "x", "uri": "https://example.org" }), false),
        (json!("https://example.org"), false),
    ] {
        assert_eq!(open.value(&value).is_ok(), ok, "{value}");
    }
    assert!(open.personal());
    for bad in [
        json!({ "photos": { "type": "folder", "max_files": 0 } }),
        json!({ "photos": { "type": "folder", "recursive": 3 } }),
        json!({ "name": { "type": "text", "default": "", "quality": { "q0": "x" } } }),
        json!({ "speed": { "type": "slider", "min": 0, "max": 2, "default": 1, "quality": { "q0": "fast" } } }),
    ] {
        assert_eq!(
            field(props(bad.clone()).check()),
            format!("properties.{}", bad.as_object().unwrap().keys().next().unwrap()),
            "{bad}"
        );
    }
    let shortcut = json!({ "open": { "type": "shortcut" } });
    let preset = Manifest {
        presets: serde_json::from_value(
            json!([{ "id": "p", "values": { "open": { "uri": "https://example.org" } } }]),
        )
        .unwrap(),
        ..props(shortcut)
    };
    assert_eq!(field(preset.check()), "presets.0.values.open");
    let quiet = props(json!({ "photos": { "type": "folder" } }));
    assert_eq!(
        serde_json::to_value(&quiet).unwrap()["properties"]["photos"],
        json!({ "type": "folder", "accept": [], "pick": "random", "every": 60 })
    );
}
