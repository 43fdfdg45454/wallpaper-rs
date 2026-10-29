mod common;

use common::{archive, image, v2};
use image::ImageFormat;
use serde_json::{Value, json};
use std::fs;
use std::io::Cursor;
use wprs_format::{Error, Kind, Limits, Manifest, Platform, features, manifest_schema, read};

fn with(kind: Kind, more: Value) -> Manifest {
    let mut value = serde_json::to_value(v2(kind)).unwrap();
    value.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
    serde_json::from_value(value).unwrap()
}

fn field(result: Result<(), Error>) -> String {
    match result {
        Err(Error::Invalid { field, .. }) => field,
        other => panic!("accepted: {other:?}"),
    }
}

fn named(manifest: &Manifest) -> Vec<String> {
    features::facts(manifest, None, None).features.into_iter().collect()
}

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

#[test]
fn texts_can_live_in_a_file_per_language() {
    let png = image(16, 16, ImageFormat::Png);
    let manifest = with(
        Kind::Image,
        json!({ "default_locale": "en", "title": "@title", "description": "@about",
        "strings": { "en": { "title": "Dusk", "about": "Hills" } }, "requires": { "features": ["locale_files"] } }),
    );
    let json = manifest.to_json().unwrap();
    let spanish = json!({ "title": "Atardecer", "about": "Colinas" }).to_string();
    let files = |extra: &[(&str, &[u8])]| {
        let base: [(&str, &[u8]); 3] =
            [("manifest.json", json.as_bytes()), ("image.png", &png), ("preview.png", &png)];
        read(Cursor::new(archive(&[&base[..], extra].concat())), &Limits::default())
    };
    let package = files(&[("locale/es.json", spanish.as_bytes())]).unwrap();
    assert_eq!(package.manifest.title_in(&["es_ES".into()]), "Atardecer");
    assert_eq!(package.manifest.locales().into_iter().collect::<Vec<_>>(), ["en", "es"]);
    assert!(package.facts.features.contains("locale_files"));
    assert!(!package.manifest.to_json().unwrap().contains("Atardecer"), "file texts stay in their file");
    let twice = files(&[("locale/en.json", spanish.as_bytes())]).map(drop);
    assert_eq!(
        match twice {
            Err(Error::Invalid { field, .. }) => field,
            other => panic!("{other:?}"),
        },
        "strings"
    );
    let named = files(&[("locale/es_ES.json", spanish.as_bytes())]).map(drop);
    assert!(matches!(named, Err(Error::Invalid { field, .. }) if field == "locale/es_ES.json"));
    let broken = files(&[("locale/es.json", b"[1]")]).map(drop);
    assert!(matches!(broken, Err(Error::Invalid { field, .. }) if field == "locale/es.json"));
    let source = tempfile::tempdir().unwrap();
    fs::create_dir(source.path().join("locale")).unwrap();
    let unlisted = with(
        Kind::Image,
        json!({ "default_locale": "en", "title": "@title", "strings": { "en": { "title": "Dusk" } } }),
    );
    fs::write(source.path().join("manifest.json"), unlisted.to_json().unwrap()).unwrap();
    [("image.png", png.as_slice()), ("preview.png", &png), ("locale/es.json", spanish.as_bytes())]
        .iter()
        .for_each(|(name, data)| fs::write(source.path().join(name), data).unwrap());
    assert!(wprs_format::fill(source.path()).unwrap().is_empty());
    let (unread, facts) = wprs_format::inspect(source.path()).unwrap();
    assert!(unread.string_files.is_empty() && facts.notes.iter().any(|n| n.contains("locale/es.json")));
    fs::write(source.path().join("manifest.json"), &json).unwrap();
    assert!(wprs_format::fill(source.path()).unwrap().is_empty());
    let (listed, facts) = wprs_format::inspect(source.path()).unwrap();
    assert_eq!(listed.title_in(&["es".into()]), "Atardecer");
    assert!(
        facts.notes.is_empty()
            && !fs::read_to_string(source.path().join("manifest.json")).unwrap().contains("Atardecer")
    );
    wprs_format::build(source.path(), source.path().join("dusk.wpak"), &Limits::default()).unwrap();
}

#[test]
fn every_feature_is_explained_documented_and_described_by_the_schema() {
    let docs = include_str!("../../../docs/specs/package.md");
    for (name, why, _) in features::all() {
        assert!(features::known(name) && features::explain(name) == *why);
        let first = |l: &&str| l.split('|').nth(1).is_some_and(|c| c.contains(&format!("`{name}`")));
        let row = |l: &&str| ["| runs |", "| without |", "| refused |"].iter().any(|e| l.ends_with(e));
        let rows = docs.lines().filter(row).filter(first).count();
        assert_eq!(rows, 1, "package.md has {rows} rows for {name}");
    }
    assert!(!features::known("holograms"));
    let schema = manifest_schema().to_string();
    for word in [
        "platforms",
        "permissions",
        "storage_kib",
        "preload",
        "packages",
        "gpu",
        "provider",
        "source_url",
        "accent",
        "flip",
        "span",
        "max_fps",
        "muted",
        "loop",
        "shortcut",
        "max_files",
        "recursive",
        "quality",
    ] {
        assert!(schema.contains(&format!("\"{word}\"")), "the schema lacks {word}");
    }
    for kind in ["web", "preset", "assets"] {
        assert!(schema.contains(&format!("\"{kind}\"")), "the schema lacks the kind {kind}");
    }
}
