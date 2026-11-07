use crate::depthwork::{Target, Work, run};
use crate::reliefwork::{Target as Shot, build, layer, outline, overlay, survey};
use image::{GrayImage, Luma, Rgb, RgbImage};
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use wprs_core::Fill;
use wprs_core::edit::{Depth, Edit, Source, Touch};
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

fn setup(depth: Depth) -> (tempfile::TempDir, Places, Shot) {
    let dir = tempfile::tempdir().unwrap();
    let image = dir.path().join("hills.png");
    let near = |x: u32, y: u32| (21..32).contains(&x) && (14..28).contains(&y);
    RgbImage::from_fn(64, 48, |x, y| Rgb(if near(x, y) { [220, 30, 30] } else { [30, 160, 50] }))
        .save(&image)
        .unwrap();
    GrayImage::from_fn(64, 48, |x, y| Luma([if near(x, y) { 230 } else { 30 }]))
        .save(wprs_depth::beside(&image))
        .unwrap();
    let places = Places { data: dir.path().join("data"), cache: dir.path().join("cache") };
    let source = Source::read(&image, [64, 48]).unwrap();
    let edit = Edit { source, depth: Some(depth), ..Edit::default() };
    let target = Shot::new(&image, &edit.to_json()).unwrap();
    (dir, places, target)
}

#[test]
fn the_depth_views_show_the_strokes_the_cuts_and_the_survey() {
    let stroke = Touch { points: vec![[0.9, 0.9]], radius: 0.05, falloff: 0.0, by: 0.9 };
    let (_dir, places, target) = setup(Depth { strokes: vec![stroke], ..Depth::default() });
    let seen = overlay(&places, &target, false).unwrap();
    assert_eq!(seen.dimensions(), (64, 48));
    assert!(seen.get_pixel(58, 43)[0] > 200 && seen.get_pixel(5, 5)[0] < 60);
    let edges = outline(&places, &target, &[0.5]).unwrap();
    assert!(edges.pixels().any(|p| p[3] == 255) && edges.get_pixel(5, 5)[3] == 40);
    let info: Value = serde_json::from_str(&survey(&places, &target).unwrap()).unwrap();
    assert_eq!(
        (info["histogram"].as_array().map(Vec::len), info["manual"].as_bool()),
        (Some(64), Some(false))
    );
    assert_eq!(
        (info["strokes"].as_u64(), info["built"].as_bool(), info["marks"].as_u64()),
        (Some(1), Some(false), Some(0))
    );
    assert!(info["cuts"].as_array().is_some_and(|c| !c.is_empty()));
}

#[test]
fn a_layer_is_built_on_demand_and_its_band_is_highlighted() {
    let cuts = Depth { cuts: Some(vec![0.5]), ..Depth::default() };
    let (_dir, places, target) = setup(cuts);
    let stop = AtomicBool::new(false);
    let path = build(&places, &target, Some(Fill::Simple), &stop, &mut |_| {}).unwrap().unwrap();
    assert!(path.is_file());
    let plain = layer(&places, &target, (0, false), Some(Fill::Simple)).unwrap();
    let marked = layer(&places, &target, (0, true), Some(Fill::Simple)).unwrap();
    assert_eq!(plain.dimensions(), (64, 48));
    assert!(plain.pixels().zip(marked.pixels()).any(|(a, b)| a != b));
    assert!(layer(&places, &target, (5, false), Some(Fill::Simple)).is_err());
    let info: Value = serde_json::from_str(&survey(&places, &target).unwrap()).unwrap();
    assert_eq!(
        (info["manual"].as_bool(), info["cuts"][0].as_f64().map(|c| (c * 10.0).round())),
        (Some(true), Some(5.0))
    );
}
