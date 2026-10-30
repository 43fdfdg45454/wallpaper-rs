use super::*;
use image::{Rgba, RgbaImage};
use wprs_scene::Tier;

fn still(at: [f32; 2], more: Value) -> Value {
    let mut s = json!({ "capacity": 8, "seed": 5, "emitters": [{ "shape": { "point": at }, "burst": { "count": 1 } }],
        "init": { "life": [30, 30], "speed": [0, 0], "size": [16, 16] } });
    more.as_object().unwrap().iter().for_each(|(k, v)| s[k] = v.clone());
    s
}

fn layer(id: &str, blend: &str) -> Value {
    json!({ "id": id, "type": "particles", "system": id, "blend": blend })
}

fn run(r: &Renderer, scene: (Value, Value, Value), images: &[(&str, RgbaImage)], gpu: bool) -> Option<Run> {
    let images = images.iter().map(|(id, i)| (*id, Pixels::new(i.clone()))).collect();
    Run::with(r, scene, images, gpu)
}

#[test]
fn trails_keep_the_path_each_particle_took() {
    let Some(r) = renderer() else { return };
    let comet = still(
        [20.0, 40.0],
        json!({ "init": { "life": [30, 30], "velocity": [[120, 0], [120, 0]], "size": [6, 6] },
        "render": { "trail": { "length": 12, "width": 1 } } }),
    );
    let scene = (json!({ "comet": comet }), json!([layer("comet", "add")]), json!({}));
    for (gpu, tier) in [false, true].into_iter().flat_map(|g| [(g, Tier::T0), (g, Tier::T3)]) {
        let Some(mut run) = run(r, scene.clone(), &[], gpu) else { return };
        run.view.set_particle_trails(r, tier);
        run.frame(r, 0.0);
        let pixels = (0..10).map(|_| run.frame(r, 1.0 / 30.0)).last().unwrap();
        let [head, middle, tail, before] = [58, 50, 42, 30].map(|x| at(&pixels, x, 40)[0]);
        assert!(head > 200 && before < 10, "gpu {gpu}: {head} {before}");
        if tier == Tier::T0 {
            assert!(middle < 10, "no trails on the lowest level, gpu {gpu}");
            continue;
        }
        assert!(middle > 40 && middle > tail, "the trail fades towards its end, gpu {gpu}: {middle} {tail}");
        looks_like("trails", &pixels);
    }
}

#[test]
fn ordered_particles_draw_the_chosen_ones_on_top() {
    let Some(r) = renderer() else { return };
    let swatch = |x: f32, order: &str| {
        still(
            [x, 40.0],
            json!({ "emitters": [{ "shape": { "point": [x, 40] }, "burst": { "count": 1, "every": 0.5 } }],
            "init": { "life": [2, 2], "speed": [0, 0], "size": [24, 24] },
            "render": { "order": order, "color_over_life": [[0, "#ff0000"], [1, "#0000ff"]] } }),
        )
    };
    let systems = json!({ "fresh": swatch(40.0, "newest"), "stale": swatch(120.0, "oldest") });
    let scene = (systems, json!([layer("fresh", "normal"), layer("stale", "normal")]), json!({}));
    for gpu in [false, true] {
        let Some(mut run) = run(r, scene.clone(), &[], gpu) else { return };
        run.frame(r, 0.0);
        let pixels = (0..36).map(|_| run.frame(r, 1.0 / 30.0)).last().unwrap();
        let ([nr, _, nb], [or, _, ob]) = (at(&pixels, 40, 40), at(&pixels, 120, 40));
        assert!(nr > nb, "the newest is on top, gpu {gpu}: {nr} {nb}");
        assert!(ob > or, "the oldest is on top, gpu {gpu}: {or} {ob}");
        looks_like("order", &pixels);
    }
}

#[test]
fn particles_bounce_on_the_opaque_part_of_a_mask() {
    let Some(r) = renderer() else { return };
    let floor =
        RgbaImage::from_fn(SIZE.0, SIZE.1, |_, y| Rgba([255, 255, 255, if y >= 50 { 255 } else { 0 }]));
    let rain = json!({ "capacity": 80, "seed": 2,
        "emitters": [{ "shape": { "box": { "from": [10, 0], "to": [150, 6] } }, "burst": { "count": 60 } }],
        "init": { "life": [30, 30], "speed": [0, 0], "size": [2, 2] },
        "forces": [{ "gravity": [0, 300] }],
        "collide": [{ "bounds": { "mask": { "texture": "floor" } }, "bounce": 0.2, "friction": 0.5 }] });
    let scene =
        (json!({ "rain": rain }), json!([layer("rain", "add")]), json!({ "floor": { "path": "floor.png" } }));
    for gpu in [false, true] {
        let Some(mut run) = run(r, scene.clone(), &[("floor", floor.clone())], gpu) else { return };
        run.frame(r, 0.0);
        let pixels = (0..45).map(|_| run.frame(r, 1.0 / 30.0)).last().unwrap();
        let c = run.census(r)[0];
        assert!(c.alive == 60 && c.max[1] < 52.0 && c.min[1] > 40.0, "gpu {gpu}: {c:?}");
        assert!((0..SIZE.0).all(|x| (56..SIZE.1).all(|y| at(&pixels, x, y) == [0, 0, 0])), "gpu {gpu}");
        looks_like("walls", &pixels);
    }
}

#[test]
fn rect_frames_gradients_and_layer_renders_match_their_reference() {
    let Some(r) = renderer() else { return };
    let sheet =
        RgbaImage::from_fn(16, 8, |x, _| if x < 8 { Rgba([255, 0, 0, 255]) } else { Rgba([0, 255, 0, 255]) });
    let sprite = |x: f32, frame: f32| {
        still(
            [x, 20.0],
            json!({ "init": { "life": [30, 30], "speed": [0, 0], "size": [12, 12], "frame": [frame, frame] },
            "render": { "texture": "sheet" } }),
        )
    };
    let mixed = json!({ "capacity": 300, "seed": 7,
        "emitters": [{ "shape": { "box": { "from": [60, 4], "to": [100, 36] } }, "burst": { "count": 300 } }],
        "init": { "life": [30, 30], "speed": [0, 0], "size": [2, 2], "gradient": [[0, "#ff0000"], [1, "#0000ff"]] } });
    let streak = still(
        [10.0, 60.0],
        json!({ "init": { "life": [30, 30], "velocity": [[60, 0], [60, 0]], "size": [6, 6] },
        "render": { "stretch": { "time": 0.2, "min": 4, "max": 30 } } }),
    );
    let systems =
        json!({ "red": sprite(20.0, 0.0), "green": sprite(140.0, 1.0), "mixed": mixed, "streak": streak });
    let layers = json!([layer("red", "normal"), layer("green", "normal"), layer("mixed", "add"), layer("streak", "add"),
        { "id": "dot", "type": "particles", "system": "streak", "blend": "add", "transform": { "position": [80, 52] },
          "render": { "color_over_life": [[0, "#00ffff"], [1, "#00ffff"]] } }]);
    let assets = json!({ "sheet": { "path": "sheet.png", "rects": [[0, 0, 8, 8], [8, 0, 8, 8]] } });
    for gpu in [false, true] {
        let Some(mut run) =
            run(r, (systems.clone(), layers.clone(), assets.clone()), &[("sheet", sheet.clone())], gpu)
        else {
            return;
        };
        run.frame(r, 0.0);
        let pixels = (0..12).map(|_| run.frame(r, 1.0 / 30.0)).last().unwrap();
        let (red, green) = (at(&pixels, 20, 20), at(&pixels, 140, 20));
        assert!(
            red[0] > 200 && red[1] < 40 && green[1] > 200 && green[0] < 40,
            "gpu {gpu}: {red:?} {green:?}"
        );
        let box_ = (60..100).flat_map(|x| (4..36).map(move |y| (x, y)));
        let tones: Vec<[u8; 3]> = box_.map(|(x, y)| at(&pixels, x, y)).filter(|p| p != &[0, 0, 0]).collect();
        let reds = tones.iter().filter(|p| u16::from(p[0]) > 2 * u16::from(p[2])).count();
        let blues = tones.iter().filter(|p| u16::from(p[2]) > 2 * u16::from(p[0])).count();
        assert!(reds > 20 && blues > 20, "the gradient picks both ends, gpu {gpu}: {reds} {blues}");
        let x = (0..SIZE.0).rev().find(|x| at(&pixels, *x, 60)[0] > 100).unwrap_or(0);
        let tail = (0..x).find(|x| at(&pixels, *x, 60)[0] > 100).unwrap_or(x);
        let dot: Vec<u32> = (0..SIZE.0).filter(|x| at(&pixels, *x, 72)[2] > 100).collect();
        assert!(
            x - tail >= 10 && (3..=8).contains(&dot.len()),
            "the system stretches and the layer draws it round, gpu {gpu}: {tail} {x} {dot:?}"
        );
        assert!(
            dot.iter().all(|x| at(&pixels, *x, 72)[0] < 40),
            "the second layer draws its own colors, gpu {gpu}"
        );
        looks_like("looks", &pixels);
    }
}
