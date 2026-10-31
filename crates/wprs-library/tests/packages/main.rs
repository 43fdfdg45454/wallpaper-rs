use rusqlite::Connection;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use wprs_format::{
    Content, Descriptor, Kind, Limits, Manifest, Origin, Previews, Rating, SCHEMA, SCHEMA_V1, Text,
};
use wprs_library::{Error, Library};

mod upgrades;

const NOW: i64 = 1_700_000_000;
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x40\0\0\0\x24\x08\x02\0\0\0";

fn package(root: &Path, name: &str, rating: Option<Rating>, flashing: bool) {
    build(root, name, rating, flashing, name.into());
}

fn build(root: &Path, name: &str, rating: Option<Rating>, flashing: bool, title: Text) {
    let content = rating.map(|rating| {
        let descriptors = if rating == Rating::Mature { vec![Descriptor::Violence] } else { Vec::new() };
        Content { rating, descriptors }
    });
    let v2 = content.is_some();
    let manifest = Manifest {
        schema: if v2 { SCHEMA } else { SCHEMA_V1 }.into(),
        id: format!("org.example.{name}"),
        version: "1.0.0".into(),
        default_locale: matches!(title, Text::Localized(_)).then(|| "en".into()),
        title,
        license: "CC0-1.0".into(),
        kind: Kind::Image,
        entry: "image.png".into(),
        content,
        photosensitive: v2.then_some(flashing),
        origin: v2.then(Origin::default),
        previews: v2.then(|| Previews { thumbnail: Some("preview.png".into()), animated: None }),
        tags: if v2 { Vec::new() } else { vec!["sea".into(), "<img src=x>".into()] },
        ..Default::default()
    };
    let source = root.parent().unwrap().join(name);
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("manifest.json"), manifest.to_json().unwrap()).unwrap();
    [("image.png", PNG), ("preview.png", PNG)]
        .iter()
        .for_each(|(f, b)| fs::write(source.join(f), b).unwrap());
    fs::create_dir_all(root).unwrap();
    wprs_format::build(&source, root.join(format!("{name}.wpak")), &Limits::default()).unwrap();
}

#[test]
fn rated_packages_are_searchable_and_confirmed_once_per_package() {
    let dir = TempDir::new().unwrap();
    let mut library = Library::open(dir.path().join("library.db")).unwrap();
    let root = dir.path().join("pictures");
    package(&root, "storm", Some(Rating::Mature), true);
    package(&root, "meadow", Some(Rating::General), false);
    package(&root, "calm", None, false);
    let source = library.add_folder(&root, true).unwrap();
    library.scan(&source, NOW).unwrap();
    let count = |library: &Library, q: &str| library.count(q, NOW).unwrap();
    let counts =
        ["content:mature", "content:general", "content:unrated", "facet:photosensitive", "-content:mature"];
    assert_eq!(counts.map(|q| count(&library, q)), [1, 1, 1, 1, 2]);
    assert!(matches!(library.count("content:pg", NOW), Err(Error::Query(_))));
    let hidden = ["", "content:mature"].map(|q| count(&library, &wprs_library::shown(q, true)));
    assert_eq!((hidden, wprs_library::shown("sea", false)), ([2, 1], "sea".into()));
    let storm = library.search("content:mature", NOW, 0, 1).unwrap().remove(0);
    let info = storm.package.clone().unwrap();
    assert_eq!((info.content.as_deref(), info.photosensitive), (Some("mature"), Some(true)));
    assert_eq!(
        (info.facets, info.confirmed),
        (
            ["android", "desktop", "landscape", "photosensitive"].map(String::from).to_vec(),
            Vec::<String>::new()
        )
    );
    assert_eq!(library.unconfirmed_flashing().unwrap(), std::slice::from_ref(&storm.uid));
    library.confirm(&[&storm.uid], "photosensitive").unwrap();
    library.confirm(&[&storm.uid, &storm.uid], "mature").unwrap();
    let confirmed = library.item(&storm.uid).unwrap().package.unwrap().confirmed;
    assert_eq!(confirmed, ["mature", "photosensitive"]);
    assert!(library.unconfirmed_flashing().unwrap().is_empty());
    assert!(matches!(library.confirm(&[&storm.uid], "spoilers"), Err(Error::Invalid(..))));
    assert!(matches!(library.confirm(&["01NOPE"], "mature"), Err(Error::NotFound(..))));
    assert_eq!(library.search("content:unrated", NOW, 0, 1).unwrap()[0].tags, ["sea"]);
    fs::copy(root.join("storm.wpak"), root.join("storm-copy.wpak")).unwrap();
    library.scan(&source, NOW).unwrap();
    let mut both: Vec<_> =
        library.search("content:mature", NOW, 0, 5).unwrap().into_iter().map(|i| i.uid).collect();
    both.sort();
    let copy = both.iter().find(|uid| **uid != storm.uid).unwrap();
    assert_eq!(library.item(copy).unwrap().package.unwrap().confirmed, ["mature", "photosensitive"]);
    assert_eq!(library.confirm(&[&storm.uid], "mature").unwrap(), both);
    let calm = library.search("content:unrated", NOW, 0, 1).unwrap().remove(0).uid;
    assert_eq!(library.confirm(&[&calm, &calm], "mature").unwrap(), [calm]);
    let id = info_id(&library, &storm.uid);
    assert!(!library.decided(&id, "motion").unwrap());
    library.confirm(&[&storm.uid], "motion").unwrap();
    assert!(library.decided(&id, "motion").unwrap());
    assert_eq!(library.unconfirm(&[&storm.uid], "motion").unwrap(), both);
    assert!(!library.decided(&id, "motion").unwrap() && library.decided(&id, "mature").unwrap());
    assert!(matches!(library.unconfirm(&["01NOPE"], "motion"), Err(Error::NotFound(..))));
}

fn info_id(library: &Library, uid: &str) -> String {
    library.item(uid).unwrap().package.unwrap().id
}
