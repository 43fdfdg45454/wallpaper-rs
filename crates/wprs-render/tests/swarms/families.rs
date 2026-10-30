use super::*;

fn dots(more: Value) -> Value {
    let mut s = json!({ "capacity": 64, "seed": 9, "emitters": [{ "shape": { "point": [20, 20] }, "burst": { "count": 1 } }],
        "init": { "life": [30, 30], "speed": [0, 0], "size": [4, 4], "color": ["#ffffff", "#ffffff"] } });
    more.as_object().unwrap().iter().for_each(|(k, v)| s[k] = v.clone());
    s
}

fn gallery() -> (Value, Value) {
    let systems = json!({
        "world": dots(json!({ "space": "world", "emitters": [{ "shape": { "point": [80, 40] }, "burst": { "count": 1 } }], "init": { "life": [30, 30], "speed": [0, 0], "size": [6, 6], "color": ["#ff0000", "#ff0000"] } })),
        "pointed": dots(json!({ "points": [{ "at": [0, 0] }, { "at": [60, 15] }],
            "emitters": [{ "shape": { "box": { "from": [-4, -4], "to": [4, 4], "inner": [6, 6], "at": "p1" } }, "burst": { "count": 40 } }],
            "init": { "life": [30, 30], "speed": [0, 0], "size": [1.5, 1.5], "color": ["#00ff00", "#00ff00"] } })),
        "half": dots(json!({ "emitters": [{ "shape": { "circle": { "center": [110, 20], "radius": 12, "inner": 11, "sign": [0, 1] } }, "burst": { "count": 60 } }],
            "init": { "life": [30, 30], "speed": [0, 0], "size": [1.5, 1.5], "hsv": { "hue": [240, 240] } } })),
        "fuse": dots(json!({ "emitters": [{ "shape": { "point": [40, 60] }, "burst": { "count": 3 } }],
            "init": { "life": [0.1, 0.1], "speed": [0, 0], "size": [3, 3] },
            "children": [{ "system": "spark", "on": "death", "offset": [60, 0], "inherit": ["color"] }] })),
        "spark": dots(json!({ "emitters": [{ "shape": { "circle": { "center": [0, 0], "radius": 10, "inner": 9 } }, "burst": { "count": 8 } }],
            "init": { "life": [30, 30], "speed": [0, 0], "size": [2, 2], "palette": { "colors": ["#ffff00"] } } }))
    });
    let layers = json!([
        { "id": "w", "type": "particles", "system": "world", "blend": "add", "transform": { "position": [100, 40], "scale": [3, 3] } },
        { "id": "p", "type": "particles", "system": "pointed", "blend": "add" },
        { "id": "h", "type": "particles", "system": "half", "blend": "add" },
        { "id": "f", "type": "particles", "system": "fuse", "blend": "add", "instance": { "color": "#ff00ff" } }
    ]);
    (systems, layers)
}

#[test]
fn new_emitters_children_and_world_space_match_their_reference() {
    let Some(r) = renderer() else { return };
    let (systems, layers) = gallery();
    for gpu in [false, true] {
        let Some(mut run) = Run::new(r, systems.clone(), layers.clone(), gpu) else { return };
        run.frame(r, 0.0);
        let pixels = (0..6).map(|_| run.frame(r, 1.0 / 30.0)).last().unwrap();
        assert!(at(&pixels, 100, 40)[0] > 200, "gpu {gpu}: {:?}", at(&pixels, 100, 40));
        assert_eq!(
            at(&pixels, 106, 40),
            [0, 0, 0],
            "world space is drawn without the layer scale, gpu {gpu}"
        );
        assert_eq!(at(&pixels, 60, 15), [0, 0, 0], "the box is hollow, gpu {gpu}");
        assert!((56..65).any(|x| at(&pixels, x, 11)[1] > 100 || at(&pixels, x, 19)[1] > 100), "gpu {gpu}");
        assert!((0..20).all(|y| at(&pixels, 110, y)[2] < 40), "only the lower half, gpu {gpu}");
        assert!(
            at(&pixels, 40, 60) == [0, 0, 0] && run.census(r)[0].alive == 24,
            "the fuse burnt, gpu {gpu}"
        );
        let sparks = (90..112)
            .flat_map(|x| (48..72).map(move |y| (x, y)))
            .filter(|(x, y)| at(&pixels, *x, *y)[0] > 100);
        assert!(sparks.count() > 10, "children burst where the fuse died, gpu {gpu}");
        looks_like("gallery", &pixels);
    }
}

#[test]
fn families_agree_on_gpu_and_cpu() {
    let Some(r) = renderer() else { return };
    let systems = json!({
        "rain": { "capacity": 600, "seed": 4, "space": "world",
            "emitters": [{ "shape": { "box": { "from": [0, 0], "to": [160, 10], "inner": [100, 4] } }, "rate": 300, "periodic": { "on": [0.3, 0.6], "off": [0.1, 0.2] } }],
            "init": { "life": [0.4, 0.8, 2], "speed": [20, 40], "angle": [80, 100], "size": [2, 3], "palette": { "colors": ["#ffffff", "#88aaff"], "noise": [0.1, 0.1, 0.1] } },
            "forces": [{ "gravity": [0, 80] }, { "oscillate": { "what": "position", "freq": [1, 2], "scale": [2, 4] } }, { "cap": 90, "window": [0, 0.2, 1, 1] }],
            "collide": [{ "bounds": "canvas", "kill": true }],
            "children": [{ "system": "splash", "on": "death", "probability": 0.5 }, { "system": "mist", "on": "follow" }] },
        "splash": { "capacity": 900, "emitters": [{ "shape": { "point": [0, 0] }, "burst": { "count": 4 } }],
            "init": { "life": [0.2, 0.4], "speed": [20, 50], "angle": [200, 340], "size": [1, 2] }, "forces": [{ "gravity": [0, 120] }] },
        "mist": { "capacity": 600, "emitters": [{ "shape": { "point": [0, 0] }, "rate": 20 }], "init": { "life": [0.2, 0.3], "speed": [0, 5] } }
    });
    let layers = json!([{ "id": "r", "type": "particles", "system": "rain", "blend": "add", "transform": { "position": [90, 40] } }]);
    let Some(mut gpu) = Run::new(r, systems.clone(), layers.clone(), true) else { return };
    let mut cpu = Run::new(r, systems, layers, false).unwrap();
    for _ in 0..40 {
        gpu.frame(r, 1.0 / 30.0);
        cpu.frame(r, 1.0 / 30.0);
    }
    let (g, c) = (gpu.census(r)[0], cpu.census(r)[0]);
    assert!(c.alive > 200, "{c:?}");
    assert!(g.alive.abs_diff(c.alive) <= c.alive / 10, "alive {g:?} {c:?}");
    for i in 0..2 {
        assert!((g.mean[i] - c.mean[i]).abs() < 4.0, "mean {g:?} {c:?}");
    }
}

#[test]
fn children_without_room_stay_cheap_on_both_paths() {
    let Some(r) = renderer() else { return };
    let point = json!({ "shape": { "point": [0, 0] }, "rate": 100000 });
    let systems = json!({
        "a": { "capacity": 5000, "emitters": [{ "shape": { "point": [80, 40] }, "rate": 100000 }],
            "init": { "life": [100, 100], "speed": [0, 0] }, "children": [{ "system": "b", "on": "follow" }] },
        "b": { "capacity": 10, "emitters": vec![point; 8], "init": { "life": [100, 100], "speed": [0, 0] } }
    });
    let layers = json!([{ "id": "a", "type": "particles", "system": "a" }]);
    for gpu in [true, false] {
        let Some(mut run) = Run::new(r, systems.clone(), layers.clone(), gpu) else { return };
        run.frame(r, 1.0 / 30.0);
        for _ in 0..2 {
            let at = std::time::Instant::now();
            run.frame(r, 1.0 / 30.0);
            assert!(at.elapsed().as_millis() < 1000, "gpu {gpu}: {:?}", at.elapsed());
        }
        let (all, slots) = (run.census(r)[0], run.view.stats().particles);
        assert!(all.alive == slots && slots > 1000, "gpu {gpu}: {all:?} in {slots}");
    }
}
