use wprs_core::layout::{Layout, Monitor, Placed, span};
use wprs_core::{Config, Fit, Place, Screen, UvRect};

fn screen(x: i32, y: i32, (width, height): (u32, u32), scale: f64, px_per_mm: f64) -> Screen {
    Screen { x, y, width, height, scale, px_per_mm, identity: String::new() }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-3
}

fn rect(uv: &UvRect) -> [f64; 4] {
    [uv.offset[0], uv.offset[1], uv.scale[0], uv.scale[1]].map(f64::from)
}

#[track_caller]
fn expect(got: [f64; 4], want: [f64; 4]) {
    assert!(got.iter().zip(want).all(|(a, b)| close(*a, b)), "got {got:?}, expected {want:?}");
}

#[test]
fn neighbours_touch_in_millimetres_with_bezels_between_them() {
    let (left, right) = (screen(0, 0, (1920, 1080), 1.0, 3.6), screen(1920, 0, (1920, 1080), 1.0, 3.6));
    let layout = Layout { bezel_mm: 10.0, ..Layout::default() };
    let monitors = layout.arrange([("DP-2", (1920, 1080), &right), ("DP-1", (1920, 1080), &left)]);
    let (w, h) = (1920.0 / 3.6, 1080.0 / 3.6);
    expect(monitors[0].mm, [0.0, 0.0, w, h]);
    expect(monitors[1].mm, [w + 20.0, 0.0, w, h]);
    let canvas = 2.0 * w + 20.0;
    let uvs = span(&monitors, ((canvas * 10.0) as u32, (h * 10.0) as u32), 1.0, [0.5, 0.5]);
    assert_eq!(uvs[0].0, "DP-1");
    expect(rect(&uvs[0].1), [0.0, 0.0, w / canvas, 1.0]);
    expect(rect(&uvs[1].1), [(w + 20.0) / canvas, 0.0, w / canvas, 1.0]);
    let below = screen(0, 1080, (1920, 1080), 1.0, 3.6);
    let stacked = Layout::default().arrange([("A", (1920, 1080), &left), ("B", (1920, 1080), &below)]);
    expect(stacked[1].mm, [0.0, h, w, h]);
}

#[test]
fn mixed_densities_keep_objects_at_the_same_physical_size() {
    let sharp = screen(0, 0, (1920, 1080), 2.0, 6.4);
    let plain = screen(1920, 0, (1920, 1080), 1.0, 3.6);
    let monitors = Layout::default().arrange([("4K", (3840, 2160), &sharp), ("HD", (1920, 1080), &plain)]);
    expect(monitors[0].mm, [0.0, 0.0, 600.0, 337.5]);
    expect(monitors[1].mm, [600.0, 18.75, 1920.0 / 3.6, 300.0]);
    let uvs = span(&monitors, (4000, 1000), 1.0, [0.5, 0.5]);
    let ([a, b], [c, d]) = ([rect(&uvs[0].1), rect(&uvs[1].1)], [600.0, 1920.0 / 3.6]);
    assert!(close(a[2] / c, b[2] / d) && close(a[3] / 337.5, b[3] / 300.0), "{a:?} {b:?}");
    assert!(close(b[0], a[0] + a[2]) && b[1] > a[1], "the right screen continues the left one");
    let tv = screen(0, 0, (1920, 1080), 1.5, 0.0);
    let fallback = Layout::default().arrange([("TV", (1920, 1080), &tv)]);
    expect(fallback[0].mm, [0.0, 0.0, 1920.0 / (96.0 / 25.4 * 1.5), 1080.0 / (96.0 / 25.4 * 1.5)]);
}

#[test]
fn configured_positions_win_and_zoom_moves_inside_the_image() {
    let mut fixed = screen(0, 0, (1920, 1080), 1.0, 3.6);
    fixed.identity = "Maker Model 1".into();
    let text =
        "[layout]\nbezel_mm = 8.0\n[layout.monitors.\"Maker Model 1\"]\nmm = [5.0, 6.0, 500.0, 280.0]\n";
    let layout = toml::from_str::<Config>(text).unwrap().layout;
    assert_eq!(
        (layout.bezel_mm, &layout.monitors["Maker Model 1"]),
        (8.0, &Placed { mm: [5.0, 6.0, 500.0, 280.0] })
    );
    assert!(!toml::to_string(&Config::default()).unwrap().contains("layout"));
    expect(layout.arrange([("DP-1", (1920, 1080), &fixed)])[0].mm, [5.0, 6.0, 500.0, 280.0]);
    let one = [Monitor { id: "A".into(), mm: [0.0, 0.0, 400.0, 200.0] }];
    expect(rect(&span(&one, (2000, 1000), 2.0, [0.5, 0.5])[0].1), [0.25, 0.25, 0.5, 0.5]);
    expect(rect(&span(&one, (2000, 1000), 2.0, [0.0, 1.0])[0].1), [0.0, 0.5, 0.5, 0.5]);
    expect(rect(&span(&one, (2000, 2000), 0.5, [0.5, 0.5])[0].1), [0.0, 0.25, 1.0, 0.5]);
    let tight = [Monitor { id: "A".into(), mm: [0.0, 0.0, 500.1, 100.0] }];
    expect(rect(&span(&tight, (1000, 10000), 1.0, [0.5, 0.5])[0].1), [0.0, 0.49, 1.0, 0.02]);
    assert!(span(&[], (10, 10), 1.0, [0.5, 0.5]).is_empty());
    let broken = Placed { mm: [f64::NAN, 0.0, 0.0, 280.0] };
    let layout = Layout { monitors: [("DP-1".into(), broken)].into(), ..Layout::default() };
    expect(layout.arrange([("DP-1", (1920, 1080), &fixed)])[0].mm, [0.0, 0.0, 1920.0 / 3.6, 300.0]);
}

#[test]
fn places_turn_into_uv_rectangles() {
    let target = (1920, 1080);
    assert_eq!(Place::Fit.uv(Fit::Fit, (100, 100), target), UvRect::compute(Fit::Fit, (100, 100), target));
    assert_eq!(
        UvRect::compute(Fit::Span, (100, 100), target),
        UvRect::compute(Fit::Fill, (100, 100), target)
    );
    expect(
        rect(&Place::Crop([0.5, 0.0, 0.5, 0.5]).uv(Fit::Fill, (3840, 2160), target)),
        [0.5, 0.0, 0.5, 0.5],
    );
    expect(
        rect(&Place::Crop([0.0, 0.0, 0.5, 1.0]).uv(Fit::Fill, (2000, 1000), target)),
        [0.0, 0.21875, 0.5, 0.5625],
    );
    let monitors = vec![Monitor { id: "A".into(), mm: [0.0, 0.0, 100.0, 100.0] }];
    let span = Place::Span { monitors, output: "B".into(), center: [0.5, 0.5], zoom: 1.0 };
    assert_eq!(span.uv(Fit::Span, (100, 50), (50, 50)), UvRect::compute(Fit::Fill, (100, 50), (50, 50)));
}
