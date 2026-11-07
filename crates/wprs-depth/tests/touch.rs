use image::{Luma, Rgb, RgbImage};
use std::sync::atomic::AtomicBool;
use wprs_core::Fill;
use wprs_core::edit::{Depth, Touch};
use wprs_depth::{
    Layers, Map, Settings, band, cut, halves, histogram, layers, lift, lift_map, marks, outlines,
};

const GREEN: [u8; 3] = [20, 160, 40];
const RED: [u8; 3] = [230, 20, 20];

fn dab(points: &[[f32; 2]], radius: f32, falloff: f32, by: f32) -> Touch {
    Touch { points: points.to_vec(), radius, falloff, by }
}

fn scene(w: u32, h: u32) -> (RgbImage, Map) {
    let near = |x: u32, y: u32| (w / 3..w / 2).contains(&x) && (h / 3..2 * h / 3).contains(&y);
    let picture = RgbImage::from_fn(w, h, |x, y| Rgb(if near(x, y) { RED } else { GREEN }));
    (picture, Map::from_fn(w, h, |x, y| Luma([if near(x, y) { 60000 } else { 6000 }])))
}

fn build(depth: &Depth) -> Layers {
    shifted(depth, 0.1)
}

fn shifted(depth: &Depth, shift: f32) -> Layers {
    let (picture, map) = scene(96, 64);
    let set = Settings { fill: Fill::Simple, count: 2, shift };
    layers(&picture, &map, (depth, &set), None, &mut |_| {}, &AtomicBool::new(false)).unwrap()
}

#[test]
fn a_stroke_lifts_the_map_most_at_its_middle_and_not_at_all_outside() {
    let mut plane = vec![0.5f32; 100 * 100];
    lift(&mut plane, (100, 100), &[dab(&[[0.5, 0.5]], 0.2, 0.5, 0.2)]);
    let at = |p: &[f32], x: usize, y: usize| p[y * 100 + x];
    assert!((at(&plane, 50, 50) - 0.7).abs() < 1e-5 && (at(&plane, 55, 50) - 0.7).abs() < 1e-5);
    assert!(at(&plane, 65, 50) > 0.5 && at(&plane, 65, 50) < 0.7);
    assert_eq!((at(&plane, 75, 50), at(&plane, 0, 0), at(&plane, 50, 85)), (0.5, 0.5, 0.5));
    lift(&mut plane, (100, 100), &[dab(&[[0.2, 0.2], [0.8, 0.2]], 0.05, 0.0, -0.9)]);
    assert!(at(&plane, 50, 20).abs() < 1e-5 && at(&plane, 50, 28) == 0.5);
    lift(&mut plane, (100, 100), &[dab(&[[0.5, 0.5]], 0.5, 0.0, 1.0)]);
    assert_eq!(at(&plane, 50, 50), 1.0);
}

#[test]
fn a_stroke_over_a_map_of_integers_matches_one_over_floats() {
    let mut map = Map::from_pixel(50, 40, Luma([32768]));
    lift_map(&mut map, &[dab(&[[0.2, 0.5], [0.8, 0.5]], 0.1, 0.5, 0.25)]);
    assert_eq!(map.get_pixel(25, 20)[0], 49152);
    assert_eq!(map.get_pixel(25, 2)[0], 32768);
    lift_map(&mut map, &[dab(&[[0.5, 0.5]], 1.0, 0.0, -1.0)]);
    assert_eq!(map.get_pixel(25, 20)[0], 0);
}

#[test]
fn cuts_make_the_layers_and_drop_the_empty_ones() {
    let ramp: Vec<f32> = (0..1000).map(|i| i as f32 / 999.0).collect();
    let parts = cut(&ramp, &[0.3, 0.6]);
    assert_eq!((parts.count(), parts.cuts.clone()), (3, vec![0.3, 0.6]));
    assert!((parts.depths[1] - 0.45).abs() < 0.01);
    let low: Vec<f32> = ramp.iter().map(|v| v * 0.2).collect();
    let dropped = cut(&low, &[0.3, 0.6]);
    assert_eq!((dropped.count(), dropped.cuts.is_empty()), (1, true));
    let upper = cut(&ramp[800..], &[0.3, 0.6, 0.9]);
    assert_eq!((upper.count(), upper.cuts), (2, vec![0.9]));
    assert_eq!(cut(&[], &[0.5]).count(), 1);
    let made = build(&Depth { cuts: Some(vec![0.5]), ..Depth::default() });
    assert_eq!((made.images.len(), made.info.cuts.clone()), (2, vec![0.5]));
    let single = build(&Depth { cuts: Some(vec![0.95]), ..Depth::default() });
    assert_eq!((single.images.len(), single.images[0].get_pixel(1, 1)[3]), (1, 255));
}

#[test]
fn a_stroke_of_the_edit_moves_where_the_layers_are_cut() {
    let none = build(&Depth::default());
    let painted = Depth { strokes: vec![dab(&[[0.1, 0.1]], 0.3, 0.0, 0.95)], ..Depth::default() };
    let lifted = build(&painted);
    let seen = |l: &Layers, x, y| l.images[1].get_pixel(x, y)[3];
    assert_eq!((seen(&none, 6, 6), seen(&lifted, 6, 6)), (0, 255));
    assert_eq!(none.images.len(), lifted.images.len());
}

#[test]
fn marks_add_to_the_band_or_keep_the_fill_out_of_it() {
    let base = shifted(&Depth::default(), 0.02);
    assert!(base.info.bands[0].pixels > 0 && base.info.bands[1].pixels == 0);
    let behind = [[40.0 / 96.0, 31.0 / 64.0]];
    let refill = Depth { fills: vec![dab(&behind, 0.04, 0.0, 1.0)], ..Depth::default() };
    let more = shifted(&refill, 0.02);
    assert!(more.info.bands[0].pixels > base.info.bands[0].pixels + 20, "{:?}", more.info.bands);
    assert_eq!(more.images[0].get_pixel(40, 31)[3], 255);
    let elsewhere = Depth { fills: vec![dab(&[[0.1, 0.1]], 0.1, 0.0, 1.0)], ..Depth::default() };
    assert_eq!(shifted(&elsewhere, 0.02).info.bands, base.info.bands);
    let protect = Depth { fills: vec![dab(&[[0.5, 0.5]], 1.0, 0.0, -1.0)], ..Depth::default() };
    assert_eq!(build(&protect).info.bands[0].pixels, 0);
    let both = [dab(&[[0.5, 0.5]], 1.0, 0.0, -1.0), dab(&[[0.1, 0.1]], 0.1, 0.0, 1.0)];
    let (refilled, protected) = marks((96, 64), &both);
    assert!(refilled[6 * 96 + 6] && !protected[6 * 96 + 6] && protected[60 * 96 + 90]);
    assert!(!refilled[60 * 96 + 90]);
}

#[test]
fn the_band_of_a_layer_is_what_the_set_filled() {
    let (picture, map) = scene(96, 64);
    let depth = Depth { fills: vec![dab(&[[0.42, 0.48]], 0.04, 0.0, 1.0)], ..Depth::default() };
    let made = shifted(&depth, 0.02);
    for layer in 0..2 {
        let found = band(&picture, &map, &depth, &made.info, layer).unwrap();
        assert_eq!(found.iter().filter(|b| **b).count(), made.info.bands[layer].pixels, "layer {layer}");
    }
    assert!(band(&picture, &map, &depth, &made.info, 2).is_err());
}

#[test]
fn the_views_show_outlines_a_histogram_and_two_halves() {
    let (_, map) = scene(96, 64);
    let shown = outlines(&map, &[0.5]);
    assert_eq!(shown.get_pixel(10, 10)[3], 40);
    assert_eq!(shown.get_pixel(31, 30)[3], 255);
    assert_ne!(shown.get_pixel(10, 10).0[..3], shown.get_pixel(40, 30).0[..3]);
    let bars = histogram(&map, 16);
    assert_eq!((bars.len(), bars[1], bars[8]), (16, 1.0, 0.0));
    assert!(bars[14] > 0.0 && bars[14] < 1.0);
    let (left, right) = (
        image::RgbaImage::from_pixel(20, 4, image::Rgba([1, 2, 3, 255])),
        image::RgbaImage::from_pixel(20, 4, image::Rgba([9, 8, 7, 255])),
    );
    let both = halves(&left, &right, 0.5);
    assert_eq!(
        (both.get_pixel(2, 1).0, both.get_pixel(17, 1).0, both.get_pixel(10, 1).0),
        ([1, 2, 3, 255], [9, 8, 7, 255], [255; 4])
    );
}
