use super::*;

#[test]
fn headers_give_size_and_length() {
    for (format, name) in [
        (ImageFormat::Png, "png"),
        (ImageFormat::Jpeg, "jpeg"),
        (ImageFormat::WebP, "webp"),
        (ImageFormat::Gif, "gif"),
    ] {
        let found = media::probe(&image(321, 123, format)).unwrap();
        assert_eq!((found.format, found.width, found.height), (name, 321, 123), "{name}");
    }
    assert_eq!(media::probe(&gif(3, 50)).map(|m| m.duration_ms), Some(1500));
    assert_eq!(
        media::probe(&animated_webp([400, 600])).map(|m| (m.width, m.height, m.duration_ms)),
        Some((100, 50, 1000))
    );
    assert_eq!(media::probe(b"RIFF\0\0\0\0WEBP"), None);
    assert_eq!(media::probe(b"plain text"), None);
}

#[test]
fn build_fills_required_features_and_checks_inputs() {
    let tmp = tempfile::tempdir().unwrap();
    let (src, out) = (tmp.path().join("src"), tmp.path().join("glow.wpak"));
    let mut manifest = v2(Kind::Scene);
    source(&src, &manifest);
    let missing = build(&src, &out, &Limits::default());
    assert!(matches!(&missing, Err(Error::Invalid { field, .. }) if field == "inputs"), "{missing:?}");
    let at = missing.unwrap_err().to_string();
    assert!(at.contains("audio at /layers/0 of scene.json"), "{at}");
    manifest.inputs = vec![Input::Audio, Input::Media];
    source(&src, &manifest);
    assert_eq!(fill(&src).unwrap(), ["inputs.audio", "inputs.media"]);
    assert!(fill(&src).unwrap().is_empty());
    build(&src, &out, &Limits::default()).unwrap();
    let package = open(&out, &Limits::default()).unwrap();
    assert_eq!(package.manifest.features(), ["inputs.audio", "inputs.media"]);
    let facets = ["1440p", "android", "animated", "audio", "desktop", "landscape", "ultrawide"];
    assert_eq!(package.facts.facets, facets);
    let unused = ["inputs declares media but nothing in the package uses it"];
    assert_eq!(features::warnings(&package.manifest, &package.facts), unused);
    fs::write(src.join("manifest.json.minisig"), "stale").unwrap();
    source(&src, &manifest);
    assert!(matches!(fill(&src), Err(Error::Invalid { field, .. }) if field == "requires.features"));
}

#[test]
fn readers_check_features_and_previews() {
    let mut manifest = v2(Kind::Image);
    let png = image(64, 64, ImageFormat::Png);
    let load = |m: &Manifest, preview: &[u8]| {
        read(Cursor::new(package(m, &[("preview.png", preview)])), &Limits::default())
    };
    assert_eq!(load(&manifest, &png).unwrap().facts.facets, ["android", "desktop"]);
    let listed = ["inputs.audio", "inputs.network", "inputs.pointer"].map(String::from).to_vec();
    let requires = Some(wprs_format::Requires { features: listed, ..Default::default() });
    let inputs = vec![Input::Audio, Input::Pointer, Input::Network];
    let claims = Manifest { inputs, requires, ..manifest.clone() };
    let claimed = load(&claims, &png).unwrap();
    assert_eq!(claimed.facts.facets, ["android", "desktop"]);
    assert_eq!(features::warnings(&claimed.manifest, &claimed.facts).len(), 3);
    let json = manifest.to_json().unwrap();
    let files = archive(&[
        ("manifest.json", json.as_bytes()),
        ("image.png", &image(1080, 1920, ImageFormat::Png)),
        ("preview.png", &png),
    ]);
    let facets = ["1080p", "android", "desktop", "portrait"];
    assert_eq!(read(Cursor::new(files), &Limits::default()).unwrap().facts.facets, facets);
    manifest.requires = Some(wprs_format::Requires { features: vec!["sound".into()], ..Default::default() });
    let found = load(&manifest, &png).unwrap();
    assert_eq!(features::warnings(&found.manifest, &found.facts).len(), 1);
    for preview in
        [image(1025, 10, ImageFormat::Png), image(10, 10, ImageFormat::Jpeg), b"not an image".to_vec()]
    {
        let result = load(&manifest, &preview);
        assert!(
            matches!(&result, Err(Error::Invalid { field, .. }) if field == "previews.thumbnail"),
            "{result:?}"
        );
    }
    manifest.previews = Some(Previews { thumbnail: None, animated: Some("loop.gif".into()) });
    let with = |gif: Vec<u8>| {
        read(Cursor::new(package(&manifest, &[("loop.gif", &gif)])), &Limits::default()).map(drop)
    };
    with(gif(3, 100)).unwrap();
    assert!(matches!(with(gif(16, 100)), Err(Error::Invalid { field, .. }) if field == "previews.animated"));
}

#[test]
fn upgrade_converts_a_version_one_source() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    source(&src, &Manifest { preview: Some("preview.png".into()), ..v1(Kind::Scene) });
    fs::write(src.join("manifest.json.minisig"), "stale").unwrap();
    let content = Content { rating: Rating::General, descriptors: Vec::new() };
    let origin = Origin::default();
    let (upgraded, _) = upgrade(&src, content.clone(), false, origin.clone()).unwrap();
    assert_eq!(
        (upgraded.inputs.as_slice(), upgraded.features()),
        (&[Input::Audio][..], &["inputs.audio".to_string()][..])
    );
    assert_eq!(
        (upgraded.thumbnail().map(String::as_str), upgraded.preview.as_ref()),
        (Some("preview.png"), None)
    );
    assert!(!src.join("manifest.json.minisig").exists());
    build(&src, tmp.path().join("up.wpak"), &Limits::default()).unwrap();
    let again = upgrade(&src, content, false, origin);
    assert!(matches!(&again, Err(Error::Invalid { field, .. }) if field == "schema"), "{again:?}");
}
