mod common;

use common::manifest;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::sync::Arc;
use wprs_format::{
    Control, Error, Kind, Limits, Manifest, Property, build, extract, open, sha256, sha256_file,
};

const FILES: [&str; 5] =
    ["manifest.json", "assets/effect.wgsl", "assets/textures/sky.png", "preview.webp", "scene.json"];

const SCENE: &str = r#"{"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32]},
    "assets": {"sky": {"path": "assets/textures/sky.png"}},
    "layers": [{"id": "sky", "type": "image", "image": "sky"}, {"id": "tint", "type": "solid", "color": {"prop": "tint"}}]}"#;

fn source(dir: &Path) -> Manifest {
    let mut manifest = manifest(Kind::Scene);
    manifest
        .properties
        .insert("tint".into(), Property::new(Control::Color { default: "#336699".into(), alpha: false }));
    fs::create_dir_all(dir.join("assets/textures")).unwrap();
    fs::write(dir.join("manifest.json"), manifest.to_json().unwrap()).unwrap();
    fs::write(dir.join("scene.json"), SCENE).unwrap();
    fs::write(dir.join("preview.webp"), b"RIFF\0\0\0\0WEBP").unwrap();
    fs::write(dir.join("assets/textures/sky.png"), vec![7u8; 5000]).unwrap();
    fs::write(dir.join("assets/effect.wgsl"), b"@fragment fn main() {}").unwrap();
    manifest
}

#[test]
fn build_read_extract_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let (src, out, dest) = (tmp.path().join("src"), tmp.path().join("sunset.wpak"), tmp.path().join("dest"));
    let manifest = source(&src);
    let digest = build(&src, &out, &Limits::default()).unwrap();
    assert_eq!(digest, sha256_file(&out).unwrap());
    assert_eq!(digest, sha256(fs::File::open(&out).unwrap()).unwrap());
    let mut package = open(&out, &Limits::default()).unwrap();
    assert_eq!(package.manifest, manifest);
    assert_eq!(package.entries.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(), FILES);
    extract(&mut package, &dest).unwrap();
    extract(&mut package, &dest).unwrap();
    for path in FILES {
        assert_eq!(fs::read(dest.join(path)).unwrap(), fs::read(src.join(path)).unwrap(), "{path}");
    }
    assert_eq!(build(&dest, tmp.path().join("again.wpak"), &Limits::default()).unwrap(), digest);
}

#[test]
fn luts_are_read_once_per_file_and_within_limits() {
    let tmp = tempfile::tempdir().unwrap();
    let (src, out) = (tmp.path().join("src"), tmp.path().join("grade.wpak"));
    source(&src);
    let corners: String = (0..8).map(|i| format!("{} {} {}\n", i & 1, (i >> 1) & 1, i >> 2)).collect();
    fs::write(src.join("assets/grade.cube"), format!("LUT_3D_SIZE 2\n{corners}")).unwrap();
    let ids: String = (0..200).map(|i| format!(r#", "g{i}": {{"path": "assets/grade.cube"}}"#)).collect();
    let sky = r#""path": "assets/textures/sky.png"}"#;
    fs::write(src.join("scene.json"), SCENE.replace(sky, &format!("{sky}{ids}"))).unwrap();
    build(&src, &out, &Limits::default()).unwrap();
    let mut package = open(&out, &Limits::default()).unwrap();
    let scene = package.scene().unwrap();
    let luts = package.luts(&scene).unwrap();
    assert!(luts.len() == 200 && luts.iter().all(|(_, lut)| Arc::ptr_eq(lut, &luts[0].1)));
    let noise = (0..(16u32 << 20) + 1).map(|i| b'0' + (i.wrapping_mul(2_654_435_761) >> 28) as u8 % 10);
    fs::write(src.join("assets/grade.cube"), noise.collect::<Vec<u8>>()).unwrap();
    build(&src, &out, &Limits::default()).unwrap();
    let mut package = open(&out, &Limits::default()).unwrap();
    let large = package.luts(&scene);
    assert!(matches!(large, Err(Error::Limit { what: "LUT size", .. })), "{large:?}");
}

#[test]
fn sha256_matches_the_reference_vector() {
    let digest = sha256(&b"abc"[..]).unwrap();
    assert_eq!(digest, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
}

#[test]
fn build_rejects_invalid_sources() {
    let tmp = tempfile::tempdir().unwrap();
    let (src, out) = (tmp.path().join("src"), tmp.path().join("out.wpak"));
    source(&src);
    symlink("/etc/hostname", src.join("assets/host.png")).unwrap();
    assert!(
        matches!(build(&src, &out, &Limits::default()), Err(Error::Symlink(p)) if p == "assets/host.png")
    );
    fs::remove_file(src.join("assets/host.png")).unwrap();
    fs::write(src.join("scene.json"), SCENE.replace("assets/textures/sky.png", "assets/gone.png")).unwrap();
    assert!(
        matches!(build(&src, &out, &Limits::default()), Err(Error::Missing(p)) if p == "assets/gone.png")
    );
    assert!(!out.exists());
    fs::write(src.join("scene.json"), SCENE.replace("\"prop\": \"tint\"", "\"prop\": \"shade\"")).unwrap();
    assert!(
        matches!(build(&src, &out, &Limits::default()), Err(Error::Invalid { field, .. }) if field == "scene")
    );
    let mut seed = 7u64;
    let mut noise = || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        char::from(b'a' + (seed >> 60) as u8)
    };
    let pad: String = (0..9 << 20).map(|_| noise()).collect();
    let padded = SCENE.replace("\"layers\"", &format!("\"ext\": {{\"x_pad\": \"{pad}\"}}, \"layers\""));
    fs::write(src.join("scene.json"), padded).unwrap();
    let large = build(&src, &out, &Limits::default());
    assert!(matches!(large, Err(Error::Limit { what: "scene size", .. })), "{large:?}");
    fs::remove_file(src.join("scene.json")).unwrap();
    assert!(matches!(build(&src, &out, &Limits::default()), Err(Error::Missing(p)) if p == "scene.json"));
    fs::remove_file(src.join("manifest.json")).unwrap();
    assert!(matches!(build(&src, &out, &Limits::default()), Err(Error::Io(_))));
    assert!(!out.exists());
}

#[test]
fn extract_refuses_symlinks_and_obstacles() {
    let tmp = tempfile::tempdir().unwrap();
    let (src, out, outside) = (tmp.path().join("src"), tmp.path().join("p.wpak"), tmp.path().join("outside"));
    source(&src);
    build(&src, &out, &Limits::default()).unwrap();
    let mut package = open(&out, &Limits::default()).unwrap();
    fs::create_dir(&outside).unwrap();
    let dest = tmp.path().join("dest");
    fs::create_dir(&dest).unwrap();
    symlink(&outside, dest.join("assets")).unwrap();
    assert!(matches!(extract(&mut package, &dest), Err(Error::Symlink(_))));
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
    let linked = tmp.path().join("linked");
    symlink(&outside, &linked).unwrap();
    assert!(matches!(extract(&mut package, &linked), Err(Error::Symlink(_))));
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
    let blocked = tmp.path().join("blocked");
    fs::create_dir_all(blocked.join("scene.json")).unwrap();
    assert!(matches!(extract(&mut package, &blocked), Err(Error::Blocked(p)) if p.ends_with("scene.json")));
    let file_link = tmp.path().join("file-link");
    fs::create_dir(&file_link).unwrap();
    symlink(outside.join("target"), file_link.join("manifest.json")).unwrap();
    assert!(matches!(extract(&mut package, &file_link), Err(Error::Symlink(_))));
    assert!(!outside.join("target").exists());
}
