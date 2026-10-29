mod common;

use common::{archive, manifest, v2};
use wprs_format::{
    Ancestor, Content, Control, Credit, Descriptor, Error, Input, Kind, Limits, Links, Manifest, OriginKind,
    Property, Rating, Requires, Result, open, thumbnail,
};

fn with(edit: impl FnOnce(&mut Manifest)) -> Result<()> {
    let mut manifest = v2(Kind::Scene);
    edit(&mut manifest);
    manifest.check()
}

fn field(result: Result<()>) -> String {
    match result {
        Err(Error::Invalid { field, .. }) => field,
        other => panic!("expected an invalid field, got {other:?}"),
    }
}

fn ancestor(author: &str) -> Ancestor {
    Ancestor { author: author.into(), title: Some("Harbor".into()), url: None, license: None }
}

#[test]
fn a_full_manifest_roundtrips() {
    let mut full = v2(Kind::Scene);
    full.content = Some(Content { rating: Rating::Mature, descriptors: vec![Descriptor::Violence] });
    full.origin.as_mut().unwrap().kind = OriginKind::Derived;
    full.origin.as_mut().unwrap().derived_from = vec![ancestor("Jo Doe")];
    full.credits = vec![Credit {
        paths: vec!["assets/".into()],
        license: "CC0-1.0".into(),
        attribution: "Jo Doe".into(),
        url: Some("https://example.org/jo".into()),
        source: None,
    }];
    full.links = Some(Links { homepage: Some("https://example.org".into()), ..Links::default() });
    full.inputs = vec![Input::Audio, Input::Network];
    full.requires = Some(Requires { features: vec!["inputs.audio".into()], ..Default::default() });
    full.check().unwrap();
    assert_eq!(Manifest::from_json(full.to_json().unwrap().as_bytes()).unwrap(), full);
}

#[test]
fn rating_photosensitivity_and_origin_are_required() {
    assert_eq!(field(with(|m| m.content = None)), "content");
    assert_eq!(field(with(|m| m.photosensitive = None)), "photosensitive");
    assert_eq!(field(with(|m| m.origin = None)), "origin");
    let mature =
        |d: Vec<Descriptor>| with(|m| m.content = Some(Content { rating: Rating::Mature, descriptors: d }));
    assert_eq!(field(mature(Vec::new())), "content.descriptors");
    assert_eq!(field(mature(vec![Descriptor::Gore, Descriptor::Gore])), "content.descriptors");
    mature(vec![Descriptor::Sexual, Descriptor::Language]).unwrap();
}

#[test]
fn origin_credits_and_links_are_checked() {
    let derived = |list: Vec<Ancestor>| {
        with(|m| m.origin.as_mut().map(|o| (o.kind, o.derived_from) = (OriginKind::Derived, list)).unwrap())
    };
    assert_eq!(field(derived(Vec::new())), "origin.derived_from");
    assert_eq!(field(derived(vec![ancestor(" ")])), "origin.derived_from");
    derived(vec![ancestor("Jo Doe")]).unwrap();
    let original = with(|m| m.origin.as_mut().unwrap().derived_from = vec![ancestor("Jo")]);
    assert_eq!(field(original), "origin.derived_from");
    let credit = |license: &str| Credit {
        paths: vec!["a.png".into()],
        license: license.into(),
        attribution: "Jo".into(),
        url: None,
        source: None,
    };
    assert_eq!(field(with(|m| m.credits = vec![credit("not spdx!")])), "credits");
    for url in ["http://example.org", "https://", "https://a b.org", "ftp://example.org"] {
        let links = Links { donate: Some(url.into()), ..Links::default() };
        assert_eq!(field(with(|m| m.links = Some(links))), "links", "{url}");
    }
    assert_eq!(field(with(|m| m.preview = Some("preview.png".into()))), "preview");
}

#[test]
fn plain_texts_cannot_carry_markup() {
    let image = r#"Bo <img src="https://tracker.example/p.gif">"#;
    let credit = Credit {
        paths: vec!["a.png".into()],
        license: "CC0-1.0".into(),
        attribution: image.into(),
        url: None,
        source: None,
    };
    assert_eq!(field(with(|m| m.credits = vec![credit])), "credits");
    let derived = |a: Ancestor| {
        with(|m| {
            m.origin.as_mut().map(|o| (o.kind, o.derived_from) = (OriginKind::Derived, vec![a])).unwrap()
        })
    };
    assert_eq!(field(derived(ancestor(image))), "origin.derived_from");
    assert_eq!(
        field(derived(Ancestor { title: Some(image.into()), ..ancestor("Jo") })),
        "origin.derived_from"
    );
    assert_eq!(field(with(|m| m.authors = vec![image.into()])), "authors");
    assert_eq!(field(with(|m| m.title = image.into())), "title");
    let labeled = Property { label: Some(image.into()), ..Property::new(Control::Bool { default: true }) };
    assert_eq!(field(with(|m| _ = m.properties.insert("glow".into(), labeled))), "properties.glow.label");
    with(|m| m.description = "Hearts <3 and a > b".into()).unwrap();
    let mut old = manifest(Kind::Image);
    old.authors = vec!["Bo <bo@example.org>".into()];
    old.check().unwrap();
}

#[test]
fn property_ids_grow_in_version_two() {
    let bool = || Property::new(Control::Bool { default: true });
    with(|m| _ = m.properties.insert("audioVolume".into(), bool())).unwrap();
    let mut old = manifest(Kind::Scene);
    old.properties.insert("audioVolume".into(), bool());
    assert_eq!(field(old.check()), "properties.audioVolume");
    let twins = with(|m| m.properties.extend([("glow".into(), bool()), ("Glow".into(), bool())]));
    assert_eq!(field(twins), "properties");
    with(|m| m.properties.extend((0..256).map(|i| (format!("p{i}"), bool())))).unwrap();
    assert_eq!(
        field(with(|m| m.properties.extend((0..257).map(|i| (format!("p{i}"), bool()))))),
        "properties"
    );
    for bad in ["1st", "with-dash", "dot.ted", "_x"] {
        assert_eq!(field(with(|m| _ = m.properties.insert(bad.into(), bool()))), format!("properties.{bad}"));
    }
}

#[test]
fn version_one_keeps_its_rules() {
    let mut old = manifest(Kind::Image);
    old.check().unwrap();
    old.photosensitive = Some(true);
    assert_eq!(field(old.check()), "schema");
    let json = manifest(Kind::Image).to_json().unwrap().replace("Sunset", "@title");
    assert_eq!(Manifest::from_json(json.as_bytes()).unwrap().title_in(&[]), "@title");
}

#[test]
fn readers_say_when_a_newer_version_is_needed() {
    let json = v2(Kind::Image).to_json().unwrap();
    let newer = |json: String| matches!(Manifest::from_json(json.as_bytes()), Err(Error::Newer(_)));
    assert!(newer(json.replace("wpak/2", "wpak/3")));
    let required = |r: &str| json.replacen("\"id\"", &format!("\"requires\": {r}, \"id\""), 1);
    assert!(newer(required(r#"{"features": ["hologram"], "colors": 9}"#)));
    assert!(newer(required(r#"{"engine": ">=99.0"}"#)));
    assert!(newer(required(r#"{"features": ["effect.no_such_effect"]}"#)));
    assert!(!newer(required(r#"{"engine": ">=0.1", "features": ["inputs.audio", "sound"]}"#)));
    let unknown = json.replacen("\"id\"", "\"hologram\": true, \"id\"", 1);
    assert!(matches!(Manifest::from_json(unknown.as_bytes()), Err(Error::Json(_))));
    let later = required(r#"{"features": ["hologram"], "colors": 9}"#);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("later.wpak");
    let files: [(&str, &[u8]); 3] =
        [("manifest.json", later.as_bytes()), ("image.png", b"entry"), ("preview.png", b"picture")];
    std::fs::write(&path, archive(&files)).unwrap();
    assert!(matches!(open(&path, &Limits::default()), Err(Error::Newer(_))));
    assert_eq!(thumbnail(&path).unwrap().as_deref(), Some(&b"picture"[..]));
}
