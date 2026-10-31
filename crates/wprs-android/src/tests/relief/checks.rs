use super::super::ask;
use crate::{depth, framing, library, store};
use serde_json::{Value, json};
use std::fs::{self, File};
use std::os::fd::AsRawFd;
use std::path::Path;

#[test]
fn cancelling_a_depth_map_answers_at_once() {
    assert_eq!(depth::request(r#"{"op": "cancel"}"#), "true");
    assert_eq!(depth::request(r#"{"op": "cancel"}"#), "false");
}

#[test]
fn the_runtime_is_taken_from_its_archive_by_checksum() {
    let dir = tempfile::tempdir().unwrap();
    let (archive, library) = (dir.path().join("runtime.aar"), b"\x7fELF a library".to_vec());
    let mut zip = zip::ZipWriter::new(File::create(&archive).unwrap());
    zip.start_file("jni/x86_64/libonnxruntime.so", zip::write::SimpleFileOptions::default()).unwrap();
    std::io::Write::write_all(&mut zip, &library).unwrap();
    zip.finish().unwrap();
    fs::write(dir.path().join("plain"), &library).unwrap();
    let sha = wprs_depth::sha256(&dir.path().join("plain")).unwrap();
    let asset = |file_sha256: &'static str| wprs_depth::Asset {
        member: "jni/x86_64/libonnxruntime.so",
        file_sha256,
        ..wprs_depth::MODEL
    };
    let dest = dir.path().join("libonnxruntime.so");
    let leak: &'static str = Box::leak(sha.into_boxed_str());
    depth::model::unpack(&asset(leak), &archive, &dest).unwrap();
    assert_eq!(fs::read(&dest).unwrap(), library);
    assert!(fs::metadata(&dest).unwrap().permissions().readonly());
    let other = dir.path().join("other.so");
    assert!(depth::model::unpack(&asset("00"), &archive, &other).is_err() && !other.exists());
    let member = wprs_depth::Asset { member: "jni/arm64-v8a/libonnxruntime.so", ..asset(leak) };
    assert!(depth::model::unpack(&member, &archive, &other).is_err());
}

pub(super) fn masked(uid: &str, image: &Path) {
    let edit = |value: Value| wprs_core::Edit::from_value(value).unwrap();
    let tinted = edit(json!({"version": 1, "geometry": {"rotate": 90},
        "masks": {"near": {"type": "depth", "range": [0.5, 1.0], "soft": 0.1}},
        "ops": [{"id": "ev", "type": "exposure", "ev": 1, "mask": "near"}]}));
    let before = depth::loaded(image).unwrap();
    depth::change(image, |e| {
        (e.geometry, e.masks, e.ops) = (tinted.geometry.clone(), tinted.masks.clone(), tinted.ops.clone());
        Ok(())
    })
    .unwrap();
    let framed = framing::framed(uid, image, wprs_core::Fit::Fill, false);
    let develop = framed.develop.as_ref().unwrap();
    let map = depth::maps::masking(framed.map.as_ref().map(|(path, _)| path.as_path()), develop);
    assert_eq!(
        (develop.needs_depth(), develop.geometry.rotate, map.map(|m| m.dimensions())),
        (true, 90, Some((32, 16))),
        "a turned photo with a depth mask is developed with its map"
    );
    depth::change(image, |e| {
        (e.geometry, e.masks, e.ops) = (before.geometry, before.masks, before.ops);
        Ok(())
    })
    .unwrap();
    assert!(framing::framed(uid, image, wprs_core::Fit::Fill, false).map.is_none());
}

pub(crate) fn heif_photos_get_maps() {
    let decode = |_: &Path, edge: u32| {
        Some(image::RgbaImage::from_pixel(edge.min(64), edge.min(32), image::Rgba([9; 4])))
    };
    assert!(store::store(&mut super::super::open("blue-64x32.heic"), "beach.heic").is_err());
    wprs_render::set_platform_decoder(wprs_render::Platform { decode, size: |_| Some((64, 32)) });
    wprs_library::platform_decodes_heif();
    let (uid, _) = store::store(&mut super::super::open("blue-64x32.heic"), "beach.heic").unwrap();
    let image = library::with(|lib| Ok(lib.item(&uid)?.path)).unwrap();
    assert_eq!(wprs_depth::decode(&image).unwrap().dimensions(), (64, 32));
    let map = tempfile::NamedTempFile::new().unwrap();
    wprs_depth::write(&wprs_depth::Map::from_pixel(64, 32, image::Luma([9])), map.path()).unwrap();
    let file = File::open(map.path()).unwrap();
    let request = json!({ "op": "import", "uid": uid, "fd": file.as_raw_fd() });
    assert_eq!(serde_json::from_str::<Value>(&depth::request(&request.to_string())).unwrap(), Value::Null);
    assert!(depth::raised(&image).and_then(|raise| raise.map).is_some());
    assert_eq!(ask(json!({ "op": "remove", "uids": [uid] })), Value::Null);
}
