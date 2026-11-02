use super::super::{Repo, init};
use super::Shown;
use crate::config::Config;
use serde_json::Value;
use std::fs;
use std::path::Path;
use wprs_format::{Limits, Manifest};

fn package(dir: &Path, id: &str) -> std::path::PathBuf {
    let src = dir.join(id);
    fs::create_dir_all(&src).unwrap();
    let manifest = Manifest {
        schema: wprs_format::SCHEMA_V1.into(),
        preview: Some("preview.png".into()),
        id: id.into(),
        version: "1.0.0".into(),
        title: "Noon".into(),
        description: "A quiet gradient".into(),
        authors: vec!["Example Studio".into()],
        license: "CC0-1.0".into(),
        kind: wprs_format::Kind::Image,
        entry: "image.png".into(),
        ..Default::default()
    };
    fs::write(src.join("manifest.json"), manifest.to_json().unwrap()).unwrap();
    image::RgbImage::from_pixel(64, 32, image::Rgb([20, 60, 120])).save(src.join("image.png")).unwrap();
    image::RgbImage::from_pixel(32, 16, image::Rgb([200, 120, 40])).save(src.join("preview.png")).unwrap();
    let out = dir.join(format!("{id}.wpak"));
    wprs_format::build(&src, &out, &Limits::default()).unwrap();
    out
}

fn release(repo: &Repo, id: &str) -> Value {
    let catalog: Value = serde_json::from_slice(&fs::read(repo.dir.join("catalog.json")).unwrap()).unwrap();
    let listing = catalog["packages"].as_array().unwrap().iter().find(|l| l["id"] == id).unwrap().clone();
    listing["versions"][0].clone()
}

#[test]
fn an_unlisted_or_hidden_approval_is_added_that_way_in_one_locked_step() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data").to_string_lossy().into_owned();
    let cfg = Config::from_vars(|key| (key == "WPRS_DATA").then(|| data.clone())).unwrap();
    init(&cfg, "Example wallpapers", false, false).unwrap();
    let repo = Repo::new(&cfg.data);
    let now = 1_700_000_000_000;
    repo.add(&package(dir.path(), "org.example.noon"), now, Shown { hidden: false, unlisted: true }).unwrap();
    assert_eq!(release(&repo, "org.example.noon")["listed"], Value::Bool(false));
    repo.add(&package(dir.path(), "org.example.dawn"), now, Shown::default()).unwrap();
    assert!(release(&repo, "org.example.dawn").get("listed").is_none_or(Value::is_null));
    repo.add(&package(dir.path(), "org.example.dusk"), now, Shown { hidden: true, unlisted: false }).unwrap();
    assert_eq!(release(&repo, "org.example.dusk")["hidden"], Value::Bool(true));
}
