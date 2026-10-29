use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

const VIEW: [f32; 4] = [0.0, 0.0, 100.0, 50.0];
const NOON: f64 = 20_755.0 * 86_400.0 + 12.0 * 3600.0;

fn runtime(layers: Json) -> Runtime {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] }, "layers": layers,
        "clock": { "step": 600 } });
    Runtime::new(Arc::new(Scene::load(&text.to_string(), &BTreeMap::new()).unwrap()), Values::new())
}

fn at(rt: &mut Runtime, t: f64) -> Frame<'_> {
    rt.seek(t);
    rt.frame(VIEW, Quality::Q3)
}

fn color(frame: &Frame) -> [f32; 4] {
    match &frame.draws[0].content {
        Content::Solid { color, .. } => color.0,
        other => panic!("{other:?}"),
    }
}

#[test]
fn colors_blend_in_oklab_along_the_shorter_hue() {
    let solid = |keys: Json| json!([{ "id": "s", "type": "solid", "color": { "keys": keys } }]);
    let mut rt = runtime(solid(json!([[0, "#ff0000"], [2, "#00ff00"]])));
    let [r, g, b, a] = color(&at(&mut rt, 1.0));
    assert!(r > 0.9 && g > 0.5 && b < 0.1 && a == 1.0, "{r} {g} {b}");
    let mut rt = runtime(solid(json!([[0, "#ff0040"], [2, "#ff4000"]])));
    let [r, g, b, _] = color(&at(&mut rt, 1.0));
    assert!(r > 0.95 && g < 0.2 && b < 0.2, "the hue went the long way: {r} {g} {b}");
    let mut rt = runtime(solid(json!([[0, "#000000"], [2, "#ffffff"]])));
    let [r, g, b, _] = color(&at(&mut rt, 1.0));
    assert!((r - g).abs() < 1e-3 && (g - b).abs() < 1e-3 && r > 0.35 && r < 0.65, "{r} {g} {b}");
    assert_eq!(color(&at(&mut rt, 2.0)), [1.0; 4]);
}

#[test]
fn handles_tracks_and_bases_shape_values() {
    let opacity = |v: Json| json!([{ "id": "s", "type": "solid", "color": "#ffffff", "opacity": v }]);
    let eased = opacity(json!({ "keys": [[0, 0, { "out": [1, 0] }], [2, 1, { "in": [1, 0] }]] }));
    let mut rt = runtime(eased);
    assert!((at(&mut rt, 1.0).draws[0].opacity - 0.5).abs() < 1e-3);
    let early = at(&mut rt, 0.4).draws[0].opacity;
    assert!(early > 0.0 && early < 0.1, "{early}");
    let mut rt = runtime(opacity(json!({ "keys": [[0, 0], [1, 0.5]], "base": 0.25 })));
    assert!((at(&mut rt, 1.0).draws[0].opacity - 0.75).abs() < 1e-4);
    let moved = json!([{ "id": "s", "type": "solid", "color": "#ffffff", "size": [10, 10], "transform": {
        "position": { "x": { "keys": [[0, 0], [2, 40]] }, "base": [5, 7] } } }]);
    let mut rt = runtime(moved);
    let world = at(&mut rt, 1.0).draws[0].world.0;
    assert!((world[4] - 20.0).abs() < 1e-3 && (world[5] - 7.0).abs() < 1e-3, "{world:?}");
    let tinted =
        json!([{ "id": "s", "type": "solid", "color": { "r": { "keys": [[0, 0], [2, 1]] }, "b": 0.5 } }]);
    let mut rt = runtime(tinted);
    let [r, g, b, a] = color(&at(&mut rt, 1.0));
    assert!((r - 0.5).abs() < 1e-3 && g == 0.0 && b == 0.5 && a == 1.0);
}

#[test]
fn time_of_day_and_dates_follow_the_local_clock() {
    let spin = |clock: &str| {
        json!([{ "id": "s", "type": "solid", "color": "#ffffff", "opacity": 0.5,
        "transform": { "rotation": { "clock": clock } } }])
    };
    let mut rt = runtime(spin("hour"));
    rt.set_wall(NOON + 125.0);
    let frame = rt.frame(VIEW, Quality::Q3);
    assert!((frame.draws[0].world.0[1] - 12f32.to_radians().sin()).abs() < 1e-4);
    assert_eq!(frame.wake, Wake::At(3475.0));
    for (clock, value, wake) in [("weekday", 4.0, 43_075.0), ("month", 10.0, 43_075.0), ("day", 0.5, 475.0)] {
        let mut rt = runtime(json!([{ "id": "s", "type": "solid", "color": "#ffffff",
            "opacity": { "clock": clock, "scale": 0.01 } }]));
        rt.set_wall(NOON + 125.0);
        let frame = rt.frame(VIEW, Quality::Q3);
        assert!((frame.draws[0].opacity - value * 0.01).abs() < 1e-5, "{clock}");
        assert_eq!(frame.wake, Wake::At(wake), "{clock}");
    }
    let (text, wake) = format_time("EEEE d 'de' MMMM yyyy, D w", NOON, "es-ES", true);
    assert_eq!((text.as_str(), wake), ("jueves 29 de octubre 2026, 302 44", Some(43_200.0)));
    let (text, wake) = format_time("EEE, MMM d ''yy h:mm a", NOON + 61.0, "en", true);
    assert_eq!((text.as_str(), wake), ("Thu, Oct 29 '26 12:01 PM", Some(59.0)));
    let new_year = 20_819.0 * 86_400.0;
    assert_eq!(format_time("yyyy-MM-dd w D", new_year, "en", true).0, "2027-01-01 53 1");
    let mut rt = runtime(json!([{ "id": "t", "type": "text", "text": { "date": "MMMM" } }]));
    rt.set_language("es");
    rt.set_wall(NOON);
    let frame = rt.frame(VIEW, Quality::Q3);
    assert!(matches!(&frame.draws[0].content, Content::Text { text, .. } if text == "octubre"));
    assert_eq!(frame.wake, Wake::At(43_200.0));
}
