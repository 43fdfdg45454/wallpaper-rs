use super::*;

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
    let docs = include_str!("../../../../docs/specs/package.md");
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
