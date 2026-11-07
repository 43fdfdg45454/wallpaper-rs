use image::{GrayImage, Luma, Rgb, RgbImage};
use std::sync::atomic::AtomicBool;
use wprs_core::Fill;
use wprs_core::edit::Depth;
use wprs_depth::{
    Layers, Map, Places, Settings, filter, indices, lama, layers, plan, push_pull, simple, split,
};

const GREEN: [u8; 3] = [20, 160, 40];
const RED: [u8; 3] = [230, 20, 20];

fn scene(w: u32, h: u32) -> (RgbImage, Map) {
    let near = |x: u32, y: u32| (w / 3..w / 2).contains(&x) && (h / 3..2 * h / 3).contains(&y);
    let picture = RgbImage::from_fn(w, h, |x, y| Rgb(if near(x, y) { RED } else { GREEN }));
    let depth = Map::from_fn(w, h, |x, y| Luma([if near(x, y) { 60000 } else { 6000 }]));
    (picture, depth)
}

#[test]
fn the_guided_filter_keeps_the_edge_of_the_guide() {
    let (w, h) = (64usize, 8usize);
    let step = |x: usize| if x < 32 { 0.1 } else { 0.9 };
    let guide: Vec<f32> = (0..w * h).map(|i| step(i % w)).collect();
    let soft = |x: f32| (((x - 32.0) / 10.0).clamp(-1.0, 1.0) + 1.0) / 2.0;
    let blurry: Vec<f32> = (0..w * h).map(|i| soft((i % w) as f32)).collect();
    let out = filter([&guide, &guide, &guide], &blurry, (w, h), (4, 1e-3));
    let row = &out[3 * w..4 * w];
    assert!(blurry[3 * w + 29] > 0.1 && blurry[3 * w + 34] < 0.9, "the input edge is soft");
    assert!(row[32] - row[31] > 0.2, "the step moved to the edge of the guide: {row:?}");
    assert!(blurry[3 * w + 32] - blurry[3 * w + 31] < 0.06 && row[56] > 0.97 && row[8] < 0.03, "{row:?}");
    let flat = vec![0.5; w * h];
    let smooth = filter([&flat, &flat, &flat], &blurry, (w, h), (4, 1e-3));
    assert!((smooth[3 * w + 29] - blurry[3 * w + 29]).abs() < 0.2, "a flat guide only smooths");
}

#[test]
fn a_depth_ramp_is_cut_into_equal_layers() {
    let ramp: Vec<f32> = (0..4000).map(|i| i as f32 / 3999.0).collect();
    let parts = split(&ramp, 4);
    assert_eq!(parts.count(), 4);
    for (cut, want) in parts.cuts.iter().zip([0.25, 0.5, 0.75]) {
        assert!((cut - want).abs() < 0.03, "{:?}", parts.cuts);
    }
    for (depth, want) in parts.depths.iter().zip([0.125, 0.375, 0.625, 0.875]) {
        assert!((depth - want).abs() < 0.03, "{:?}", parts.depths);
    }
    let index = indices(&ramp, &parts);
    assert_eq!((index[0], index[3999], index[1500]), (0, 3, 1));
    assert_eq!(split(&vec![0.4; 100], 5).count(), 1);
    let steps: Vec<f32> = (0..3000).map(|i| [0.0, 0.5, 1.0][i % 3]).collect();
    assert_eq!(split(&steps, 6).count(), 3);
}

#[test]
fn the_simple_fill_never_lets_the_foreground_bleed_into_the_band() {
    let (picture, _) = scene(96, 64);
    let known: Vec<bool> = picture.pixels().map(|p| p.0 == GREEN).collect();
    let filled = simple(&picture, &known);
    for (p, k) in filled.pixels().zip(&known) {
        assert!(p.0.iter().zip(GREEN).all(|(a, b)| a.abs_diff(b) <= 2), "{:?} known {k}", p.0);
    }
    let pulled = push_pull(&picture, &known);
    assert_eq!(pulled.get_pixel(40, 30).0, GREEN);
}

fn stack(fill: Fill) -> Layers {
    let (picture, depth) = scene(96, 64);
    let set = Settings { fill, count: 2, shift: 0.1 };
    layers(&picture, &depth, (&Depth::default(), &set), None, &mut |_| {}, &AtomicBool::new(false)).unwrap()
}

#[test]
fn the_background_layer_holds_background_behind_the_near_object() {
    let made = stack(Fill::Simple);
    assert_eq!(made.images.len(), 2);
    let (back, front) = (&made.images[0], &made.images[1]);
    for (x, y) in [(34, 30), (40, 32), (47, 40)] {
        let p = back.get_pixel(x, y).0;
        assert_eq!(p[3], 255, "the background is opaque everywhere");
        assert!(p[..3].iter().zip(GREEN).all(|(a, b)| a.abs_diff(b) <= 3), "({x},{y}): {p:?}");
    }
    assert_eq!(front.get_pixel(40, 30).0, [RED[0], RED[1], RED[2], 255]);
    assert_eq!(front.get_pixel(5, 5).0[3], 0);
    assert!(made.info.depths[0] < 0.2 && made.info.depths[1] > 0.8, "{:?}", made.info.depths);
    assert!(made.info.bands[0].pixels > 0 && made.info.bands[0].reach > made.info.bands[1].reach);
}

#[test]
fn the_cache_key_follows_the_picture_the_mode_and_the_settings() {
    let dir = tempfile::tempdir().unwrap();
    let places = Places { data: dir.path().join("d"), cache: dir.path().join("c") };
    let sha = "ab".repeat(32);
    let image = dir.path().join("p.png");
    scene(8, 8).0.save(&image).unwrap();
    wprs_depth::write(&scene(8, 8).1, &wprs_depth::beside(&image)).unwrap();
    let (depth, set) = (Depth::default(), Settings { fill: Fill::Simple, count: 5, shift: 0.02 });
    let key = |sha: &str, depth: &Depth, set: &Settings| places.layer_set(&image, sha, depth, set).unwrap();
    let first = key(&sha, &depth, &set);
    assert_eq!(first, key(&sha, &depth, &set));
    assert!(
        first.to_string_lossy().ends_with(".layers.png") && first.starts_with(dir.path().join("c/depth"))
    );
    let others = [
        key(&"cd".repeat(32), &depth, &set),
        key(&sha, &Depth { gamma: 2.0, ..depth.clone() }, &set),
        key(&sha, &depth, &Settings { fill: Fill::Lama, ..set }),
        key(&sha, &depth, &Settings { count: 4, ..set }),
        key(&sha, &depth, &Settings { shift: 0.03, ..set }),
    ];
    assert!(others.iter().all(|o| *o != first) && (1..5).all(|i| others[i] != others[i - 1]));
    assert_eq!(places.layer_set(&image, "not a hash", &depth, &set), None);
}

#[test]
fn a_saved_layer_set_comes_back_and_is_trimmed_with_its_cache() {
    let dir = tempfile::tempdir().unwrap();
    let made = stack(Fill::Simple);
    let path = dir.path().join(format!("{}-0123456789abcdef.layers.png", "ab".repeat(32)));
    wprs_depth::save(&path, &made).unwrap();
    let back = wprs_depth::load(&path).unwrap().unwrap();
    assert_eq!(back.info, made.info);
    assert!(back.images.iter().zip(&made.images).all(|(a, b)| a == b));
    let places = Places { data: dir.path().join("d"), cache: dir.path().to_path_buf() };
    std::fs::rename(&path, dir.path().join("x")).unwrap();
    std::fs::create_dir_all(dir.path().join("depth")).unwrap();
    let inside = dir.path().join("depth").join(path.file_name().unwrap());
    wprs_depth::save(&inside, &made).unwrap();
    assert_eq!(places.layers_of(&"ab".repeat(32)).len(), 1);
    assert_eq!(places.unlayer(&"ab".repeat(32)), 1);
    assert!(places.layers_of(&"ab".repeat(32)).is_empty() && wprs_depth::load(&inside).unwrap().is_none());
}

#[test]
fn the_inpainter_only_touches_the_band_and_runs_a_few_times() {
    let mut image = RgbImage::from_pixel(1600, 900, Rgb([10, 10, 10]));
    let band: Vec<bool> = (0..1600 * 900).map(|i| (i % 1600 > 700) && (i % 1600 < 740)).collect();
    assert!(plan((1600, 900), &band).len() <= 8);
    let runs = std::cell::Cell::new(0);
    let mut paint = |_: &RgbImage, mask: &GrayImage| {
        runs.set(runs.get() + 1);
        assert!(mask.pixels().any(|p| p[0] == 255));
        Ok(RgbImage::from_pixel(512, 512, Rgb([200, 100, 50])))
    };
    lama(&mut image, &band, &mut paint, &AtomicBool::new(false)).unwrap();
    assert!(runs.get() >= 1);
    assert_eq!(image.get_pixel(720, 450).0, [200, 100, 50]);
    assert_eq!(image.get_pixel(100, 100).0, [10, 10, 10]);
    assert_eq!(image.get_pixel(1200, 800).0, [10, 10, 10]);
    let stopped = AtomicBool::new(true);
    assert!(lama(&mut image, &band, &mut paint, &stopped).is_err());
}
