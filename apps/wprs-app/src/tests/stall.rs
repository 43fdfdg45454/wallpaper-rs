use crate::stall::{added, changed, published, queued};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use wprs_format::stall::{featured, listed, opened};
use wprs_format::{
    Kind, Limits, Manifest, SCHEMA_V1, Settings, Signer, Text, build, init, keygen, sign_root,
};

fn package(dir: &Path, id: &str) -> PathBuf {
    let src = dir.join(format!("src-{id}"));
    fs::create_dir_all(&src).unwrap();
    let manifest = Manifest {
        schema: SCHEMA_V1.into(),
        id: id.into(),
        version: "1.0.0".into(),
        title: Text::Plain("Sky".into()),
        description: "A calm gradient".into(),
        license: "CC0-1.0".into(),
        kind: Kind::Image,
        entry: "image.png".into(),
        ..Default::default()
    };
    fs::write(src.join("manifest.json"), manifest.to_json().unwrap()).unwrap();
    fs::write(src.join("image.png"), id).unwrap();
    let out = dir.join(format!("{id}.wpak"));
    build(&src, &out, &Limits::default()).unwrap();
    out
}

#[test]
fn a_static_repository_is_opened_filled_unlisted_and_featured_from_the_app() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("repo");
    assert!(opened(&dir).is_err());
    let (root, online) = (keygen(None).unwrap(), keygen(None).unwrap());
    let settings = Settings {
        name: "Example wallpapers".into(),
        root_keys: vec![root.public],
        online_keys: vec![online.public],
        ..Settings::default()
    };
    init(&dir, &settings, 1_800_000_000).unwrap();
    assert_eq!(opened(&dir).unwrap()["packages"], serde_json::json!([]));
    let files = [package(tmp.path(), "org.example.dawn"), package(tmp.path(), "org.example.dusk")];
    let state = added(&dir, &files, true).unwrap();
    assert_eq!(state["added"], serde_json::json!(["org.example.dawn 1.0.0", "org.example.dusk 1.0.0"]));
    assert_eq!(state["name"], "Example wallpapers");
    assert_eq!(state["packages"][0]["versions"][0]["listed"], false);
    let state = changed(listed(&dir, "org.example.dawn", None, true)).unwrap();
    assert_eq!(state["changed"], true);
    assert_eq!(state["packages"][0]["versions"][0]["listed"], true);
    let state = featured(&dir, "org.example.dusk", Some("Calm evening"), true).unwrap();
    assert_eq!(
        (&state["packages"][1]["featured"], &state["packages"][1]["note"]),
        (&true.into(), &"Calm evening".into())
    );
    assert!(featured(&dir, "org.example.none", None, true).is_err());
    assert!(listed(&dir, "org.example.dawn", Some("9.0.0"), false).is_err());
    let state = featured(&dir, "org.example.dusk", None, false).unwrap();
    assert_eq!(state["packages"][1]["featured"], false);
}

#[test]
fn a_static_repository_is_published_from_the_app_with_an_online_key() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("repo");
    let (root, online) = (keygen(None).unwrap(), keygen(Some("example".into())).unwrap());
    let settings = Settings {
        root_keys: vec![root.public.clone()],
        online_keys: vec![online.public],
        ..Settings::default()
    };
    init(&dir, &settings, 1_800_000_000).unwrap();
    let (root_file, online_file) = (tmp.path().join("root.key"), tmp.path().join("online.key"));
    fs::write(&root_file, &root.secret).unwrap();
    fs::write(&online_file, &online.secret).unwrap();
    added(&dir, &[package(tmp.path(), "org.example.dawn")], false).unwrap();
    assert!(published(&dir, &online_file, Some("example".into()), 7).is_err());
    sign_root(&dir, &Signer::new(&root.secret, None).unwrap()).unwrap();
    assert!(published(&dir, &root_file, None, 7).is_err());
    assert!(published(&dir, &online_file, Some("wrong".into()), 7).is_err());
    let state = published(&dir, &online_file, Some("example".into()), 7).unwrap();
    assert_eq!(state["published"]["packages"], 1);
    assert_eq!(state["packages"][0]["id"], "org.example.dawn");
}

#[test]
fn a_catalog_change_from_the_app_waits_for_the_one_that_runs() {
    let (entered, enter) = mpsc::channel();
    let (release, wait) = mpsc::channel::<()>();
    let first = std::thread::spawn(move || {
        queued(|| {
            entered.send(()).unwrap();
            wait.recv().unwrap();
        })
    });
    enter.recv().unwrap();
    let (finished, done) = mpsc::channel();
    let second = std::thread::spawn(move || queued(|| finished.send(()).unwrap()));
    assert!(done.recv_timeout(Duration::from_millis(300)).is_err(), "two catalog changes ran at once");
    release.send(()).unwrap();
    done.recv_timeout(Duration::from_secs(10)).unwrap();
    first.join().unwrap();
    second.join().unwrap();
}
