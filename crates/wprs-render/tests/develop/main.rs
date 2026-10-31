use image::{Rgba, RgbaImage};
use serde_json::json;
use wprs_core::{Edit, Fit, UvRect};
use wprs_render::{Layer, Renderer};

mod blur;

fn renderer() -> Option<&'static Renderer> {
    match Renderer::shared() {
        Ok(r) => Some(r),
        Err(e) if std::env::var_os("WPRS_REQUIRE_GPU").is_none() => {
            eprintln!("skipping GPU test: {e}");
            None
        }
        Err(e) => panic!("GPU required but unavailable: {e}"),
    }
}

fn edit(value: serde_json::Value) -> Edit {
    let mut value = value;
    value["version"] = json!(1);
    Edit::from_value(value).unwrap()
}

fn op(kind: serde_json::Value) -> Edit {
    let mut kind = kind;
    kind["id"] = json!("op");
    edit(json!({"ops": [kind]}))
}

fn develop(r: &Renderer, image: &RgbaImage, edit: &Edit, size: (u32, u32)) -> RgbaImage {
    r.read_image(&r.develop(image, edit, None, size).unwrap()).unwrap()
}

fn flat(color: [u8; 3]) -> RgbaImage {
    RgbaImage::from_pixel(16, 16, Rgba([color[0], color[1], color[2], 255]))
}

fn at(image: &RgbaImage, x: u32, y: u32) -> [u8; 3] {
    let Rgba([r, g, b, _]) = *image.get_pixel(x, y);
    [r, g, b]
}

fn linear(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn encoded(c: f64) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let e = if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (e * 255.0).round() as u8
}

fn near(got: [u8; 3], want: [u8; 3]) -> bool {
    got.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= 2)
}

#[track_caller]
fn expect(got: [u8; 3], want: [u8; 3]) {
    assert!(near(got, want), "got {got:?}, expected {want:?}");
}

#[test]
fn neutral_edits_keep_every_pixel() {
    let Some(r) = renderer() else { return };
    let image =
        RgbaImage::from_fn(32, 16, |x, y| Rgba([(x * 8) as u8, (y * 16) as u8, 200 - (x * 3) as u8, 255]));
    let neutral = op(json!({"type": "white_balance", "temperature": 6504, "tint": 0}));
    let out = develop(r, &image, &neutral, (32, 16));
    for (x, y) in [(0, 0), (31, 15), (7, 9), (20, 3)] {
        expect(at(&out, x, y), at(&image, x, y));
    }
}

#[test]
fn point_operations_follow_their_formulas() {
    let Some(r) = renderer() else { return };
    let check = |color: [u8; 3], kind: serde_json::Value, want: [u8; 3]| {
        let out = develop(r, &flat(color), &op(kind.clone()), (8, 8));
        assert!(near(at(&out, 4, 4), want), "{kind}: got {:?}, expected {want:?}", at(&out, 4, 4));
    };
    let gray = linear(100);
    check([100; 3], json!({"type": "exposure", "ev": 1.0}), [encoded(gray * 2.0); 3]);
    let contrast = 0.18 * (gray / 0.18).powf(1.5f64.powf(0.5));
    check([100; 3], json!({"type": "contrast", "amount": 50}), [encoded(contrast); 3]);
    let luma = 0.2126 * linear(200) + 0.7152 * linear(100) + 0.0722 * linear(50);
    check([200, 100, 50], json!({"type": "saturation", "saturation": -100}), [encoded(luma); 3]);
    check([30, 140, 220], json!({"type": "curves", "rgb": [[0, 1], [1, 0]]}), [225, 115, 35]);
    check([30, 140, 220], json!({"type": "curves", "g": [[0, 0], [0.5, 1], [1, 1]]}), [30, 255, 220]);
    check([255; 3], json!({"type": "overlay", "color": "#808080", "blend": "multiply"}), [128; 3]);
    check([0, 255, 0], json!({"type": "overlay", "color": "#ff0000", "opacity": 0.5}), [128, 128, 0]);
    let over = |blend: &str| json!({"type": "overlay", "color": "#3080c0", "blend": blend, "opacity": 0.7});
    check([200, 100, 50], over("screen"), [207, 154, 158]);
    check([200, 100, 50], over("overlay"), [176, 100, 68]);
    check([200, 100, 50], over("soft_light"), [181, 100, 64]);
    check([200, 100, 50], over("color"), [95, 121, 151]);
    let warm =
        develop(r, &flat([128; 3]), &op(json!({"type": "white_balance", "temperature": 3000})), (8, 8));
    let [red, _, blue] = at(&warm, 4, 4);
    assert!(blue > red + 40, "a low temperature cools the picture: {:?}", at(&warm, 4, 4));
    let tinted = develop(
        r,
        &flat([128; 3]),
        &op(json!({"type": "white_balance", "temperature": 6504, "tint": 60})),
        (8, 8),
    );
    let [red, green, _] = at(&tinted, 4, 4);
    assert!(red > green + 10, "a positive tint adds magenta: {:?}", at(&tinted, 4, 4));
}

#[test]
fn vignette_darkens_the_corners_only() {
    let Some(r) = renderer() else { return };
    let kind = json!({"type": "vignette", "amount": -1, "midpoint": 0.3, "roundness": 0, "feather": 0.5});
    let out = develop(r, &flat([200; 3]), &op(kind), (32, 16));
    expect(at(&out, 16, 8), [200; 3]);
    assert!(at(&out, 0, 0)[0] < 60, "corner {:?}", at(&out, 0, 0));
}

#[test]
fn masks_limit_where_an_operation_applies() {
    let Some(r) = renderer() else { return };
    let masked = |mask: serde_json::Value| {
        let value =
            json!({"ops": [{"id": "dark", "type": "exposure", "ev": -5, "mask": "m"}], "masks": {"m": mask}});
        develop(r, &flat([200; 3]), &edit(value), (32, 32))
    };
    let dark = encoded(linear(200) / 32.0);
    let out = masked(json!({"type": "linear", "from": [0.0, 0.5], "to": [1.0, 0.5]}));
    expect(at(&out, 0, 16), [200; 3]);
    expect(at(&out, 31, 16), [dark; 3]);
    let out = masked(json!({"type": "linear", "from": [0.0, 0.5], "to": [1.0, 0.5], "invert": true}));
    expect(at(&out, 0, 16), [dark; 3]);
    let out = masked(json!({"type": "radial", "center": [0.5, 0.5], "radius": [0.25, 0.25], "feather": 0.1}));
    expect(at(&out, 16, 16), [dark; 3]);
    expect(at(&out, 1, 1), [200; 3]);
    let out = masked(json!({"type": "luminance", "range": [0.9, 1.0], "soft": 0.05}));
    expect(at(&out, 16, 16), [200; 3]);
    let stroke = json!({"radius": 0.1, "hardness": 0.9, "points": [[0.5, 0.5], [0.5, 0.6]]});
    let out = masked(json!({"type": "brush", "strokes": [stroke]}));
    expect(at(&out, 16, 17), [dark; 3]);
    expect(at(&out, 2, 2), [200; 3]);
}

#[test]
fn geometry_rotates_flips_and_crops() {
    let Some(r) = renderer() else { return };
    let colors = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]];
    let image = RgbaImage::from_fn(64, 32, |x, y| {
        let [r, g, b] = colors[usize::from(x >= 32) + 2 * usize::from(y >= 16)];
        Rgba([r, g, b, 255])
    });
    let corners = |geometry: serde_json::Value, size: (u32, u32)| {
        let out = develop(r, &image, &edit(json!({"geometry": geometry})), size);
        let (w, h) = (size.0 - 2, size.1 - 2);
        [(1, 1), (w, 1), (1, h), (w, h)].map(|(x, y)| at(&out, x, y))
    };
    let check = |geometry: serde_json::Value, size, order: [usize; 4]| {
        let got = corners(geometry.clone(), size);
        assert!(got.iter().zip(order).all(|(g, i)| near(*g, colors[i])), "{geometry}: {got:?}");
    };
    check(json!({"rotate": 90}), (32, 64), [2, 0, 3, 1]);
    check(json!({"rotate": 270}), (32, 64), [1, 3, 0, 2]);
    check(json!({"flip": [true, false]}), (64, 32), [1, 0, 3, 2]);
    check(json!({"crop": [0.5, 0.5, 0.5, 0.5]}), (32, 16), [3; 4]);
    let frame = |geometry: serde_json::Value| edit(json!({"geometry": geometry})).geometry.frame((64, 32));
    assert_eq!(frame(json!({"rotate": 90, "crop": [0, 0, 0.5, 1]})), (16, 64));
    let (w, h) = frame(json!({"straighten": 10}));
    assert!(w < 64 && h < 32 && (w as f32 / h as f32 - 2.0).abs() < 0.1, "{w}x{h}");
}

#[test]
fn the_daemon_draw_and_the_export_produce_the_same_pixels() {
    let Some(r) = renderer() else { return };
    let image = RgbaImage::from_fn(40, 24, |x, y| Rgba([(x * 6) as u8, (y * 10) as u8, 90, 255]));
    let edit = edit(
        json!({"ops": [{"id": "ev", "type": "exposure", "ev": 0.4}, {"id": "s", "type": "saturation", "vibrance": 30}]}),
    );
    let developed = r.develop(&image, &edit, None, (40, 24)).unwrap();
    let exported = r.read_image(&developed).unwrap();
    let mut target = r.create_target(40, 24).unwrap();
    let layer = Layer {
        image: Some(&developed),
        fit: Fit::Stretch,
        background: [0; 3],
        opacity: 1.0,
        uv: None,
        relief: None,
    };
    let mut drawn = Vec::new();
    r.render_layers(&mut target, &[layer], &mut drawn).unwrap();
    for (i, pixel) in exported.pixels().enumerate() {
        let [b, g, rr] = [drawn[i * 4], drawn[i * 4 + 1], drawn[i * 4 + 2]];
        assert!(near([rr, g, b], [pixel[0], pixel[1], pixel[2]]), "pixel {i}");
    }
    let uv = UvRect::compute(Fit::Fill, (4000, 2000), (1920, 1080));
    assert_eq!(r.develop_size((4000, 2000), uv, (1920, 1080)), (2160, 1080));
    assert_eq!(r.develop_size((1000, 500), uv, (1920, 1080)), (1000, 500));
}
