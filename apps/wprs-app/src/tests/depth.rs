use crate::depthwork::{Target, Work, run};
use image::{GrayImage, Luma, Rgb, RgbImage};
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use wprs_core::edit::Depth;
use wprs_depth::Places;

#[test]
fn depth_work_reports_assets_maps_and_failures() {
    let dir = tempfile::tempdir().unwrap();
    let places = Places { data: dir.path().join("data"), cache: dir.path().join("cache") };
    let image = dir.path().join("dunes.png");
    RgbImage::from_pixel(8, 4, Rgb([90, 80, 70])).save(&image).unwrap();
    let target = || Target {
        image: image.clone(),
        sha: String::new(),
        depth: Depth::default(),
        device: Default::default(),
    };
    let (stop, mut stages) = (AtomicBool::new(false), Vec::new());
    let (shown, failed) = run(&places, target(), Work::Open, &stop, &mut |s, _| stages.push(s.to_owned()));
    let assets: Vec<Value> = serde_json::from_str(&shown.assets).unwrap();
    assert_eq!(
        (assets[0]["id"].as_str(), assets[0]["size"].as_str()),
        (Some("depth-anything-v2-small"), Some("99.1 MB"))
    );
    assert!(!shown.ready && shown.map.is_empty() && shown.sha.len() == 64 && failed.is_none());
    let own = dir.path().join("own.png");
    GrayImage::from_pixel(8, 4, Luma([200])).save(&own).unwrap();
    let (shown, failed) = run(&places, target(), Work::Adopt(own.clone()), &stop, &mut |_, _| {});
    assert!(failed.is_none() && shown.map.ends_with("dunes.png.depth.png"), "{failed:?} {}", shown.map);
    let (grey, red) = (Depth { near: 0.5, ..Depth::default() }, Depth::default());
    for (depth, colour, pixel) in [(grey, false, [100u8, 100, 100]), (red, true, [251, 104, 25])] {
        let look = crate::depthwork::look(&places, (&image, &shown.sha), &depth, colour).unwrap();
        let bytes = std::fs::read(&look).unwrap();
        std::fs::remove_file(&look).unwrap();
        assert!(bytes.starts_with(b"P6\n8 4\n255\n"));
        let shown = RgbImage::from_raw(8, 4, bytes[11..].to_vec()).unwrap();
        let near = shown.get_pixel(4, 2).0.iter().zip(pixel).all(|(a, b)| a.abs_diff(b) <= 3);
        assert!(near && shown.dimensions() == (8, 4), "{:?} {:?}", shown.get_pixel(4, 2), shown.dimensions());
    }
    let (_, failed) = run(&places, target(), Work::Install(own), &stop, &mut |_, _| {});
    assert!(failed.unwrap().contains("neither the depth model nor ONNX Runtime"));
    assert!(stages.is_empty());
}
