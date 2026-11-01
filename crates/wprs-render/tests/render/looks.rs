use super::{close, renderer, rgb};
use image::{Rgba, RgbaImage};
use wprs_core::filter::{Filter, LOOKS};
use wprs_core::{Fit, Grade};
use wprs_render::{Layer, Renderer};

fn shot(r: &Renderer, image: &RgbaImage, flip: [bool; 2], grade: Option<Grade>, size: (u32, u32)) -> Vec<u8> {
    let uploaded = r.upload_rgba(image.as_raw(), image.width(), image.height(), image.width() * 4).unwrap();
    let mut target = r.create_target(size.0, size.1).unwrap();
    let layer = Layer { flip, grade, ..super::layer(Some(&uploaded), Fit::Stretch, [0; 3], 1.0) };
    let mut pixels = Vec::new();
    r.render_layers(&mut target, &[layer], &mut pixels).unwrap();
    pixels
}

fn linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn encoded(c: f32) -> u8 {
    let c = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (c.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[test]
fn flipping_mirrors_the_picture_across_each_axis() {
    let Some(r) = renderer() else { return };
    let mut quad = RgbaImage::new(2, 2);
    let colors = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]];
    for (i, [cr, cg, cb]) in colors.into_iter().enumerate() {
        quad.put_pixel(i as u32 % 2, i as u32 / 2, Rgba([cr, cg, cb, 255]));
    }
    let corner = |px: &[u8], x, y| rgb(px, 8, x, y);
    let plain = shot(r, &quad, [false; 2], None, (8, 8));
    assert_eq!((corner(&plain, 0, 0), corner(&plain, 7, 7)), ([255, 0, 0], [255, 255, 255]));
    let across = shot(r, &quad, [true, false], None, (8, 8));
    assert_eq!((corner(&across, 0, 0), corner(&across, 7, 0)), ([0, 255, 0], [255, 0, 0]));
    let down = shot(r, &quad, [false, true], None, (8, 8));
    assert_eq!((corner(&down, 0, 0), corner(&down, 0, 7)), ([0, 0, 255], [255, 0, 0]));
    let both = shot(r, &quad, [true; 2], None, (8, 8));
    assert_eq!(corner(&both, 0, 0), [255, 255, 255]);
}

#[test]
fn every_look_matches_its_reference_and_no_filter_changes_nothing() {
    let Some(r) = renderer() else { return };
    let swatch: [[u8; 3]; 4] = [[200, 120, 60], [30, 90, 210], [128, 128, 128], [250, 240, 10]];
    let mut strip = RgbaImage::new(4, 1);
    swatch
        .iter()
        .enumerate()
        .for_each(|(i, [a, b, c])| strip.put_pixel(i as u32, 0, Rgba([*a, *b, *c, 255])));
    let plain = shot(r, &strip, [false; 2], None, (4, 1));
    assert!((0..4).all(|x| rgb(&plain, 4, x, 0) == swatch[x as usize]));
    let tuned = Filter {
        exposure: 0.5,
        contrast: 0.3,
        saturation: -0.4,
        warmth: 0.5,
        hue: 40.0,
        ..Filter::default()
    };
    let filters =
        LOOKS.iter().map(|&look| Filter { look, strength: 0.8, ..Filter::default() }).chain([tuned]);
    for filter in filters.filter(|f| !f.is_identity()) {
        let grade = filter.grade().unwrap();
        let drawn = shot(r, &strip, [false; 2], Some(grade), (4, 1));
        for (x, c) in swatch.iter().enumerate() {
            let want = grade.apply(c.map(linear)).map(encoded);
            let got = rgb(&drawn, 4, x as u32, 0);
            assert!(close(got, want), "{filter:?} turned {c:?} into {got:?}, not {want:?}");
        }
    }
    assert!(Filter::default().grade().is_none() && Filter { strength: 0.0, ..tuned }.grade().is_some());
}
