use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_core::Fit;
use wprs_render::{Layer, Renderer, SceneAssets, SceneView};
use wprs_scene::{Census, Quality, Runtime, SCHEMA, Scene, Values};

const SIZE: (u32, u32) = (160, 80);

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

struct Run {
    runtime: Runtime,
    view: SceneView,
}

impl Run {
    fn new(r: &Renderer, systems: Value, layers: Value, gpu: bool) -> Option<Run> {
        let canvas = json!({ "size": [SIZE.0, SIZE.1], "background": "#000000" });
        let assets = json!({ "pair": { "path": "pair.png", "atlas": [1, 2] } });
        let text = json!({ "schema": SCHEMA, "canvas": canvas, "layers": layers, "particles": systems, "assets": assets });
        let scene = Scene::load(&text.to_string(), &BTreeMap::new()).unwrap_or_else(|e| panic!("{e}"));
        let pair =
            RgbaImage::from_fn(2, 1, |x, _| Rgba(if x == 0 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }));
        let images = [("pair".to_string(), wprs_render::Pixels::new(pair))].into();
        let mut view =
            r.scene_view(&scene, &SceneAssets { images, ..Default::default() }, &Values::new()).unwrap();
        if !view.set_gpu_particles(r, gpu) {
            eprintln!("skipping: no compute shaders on {}", r.adapter());
            return None;
        }
        assert_eq!(view.gpu_particles(), Some(gpu));
        Some(Run { runtime: Runtime::new(Arc::new(scene), Values::new()), view })
    }

    fn frame(&mut self, r: &Renderer, dt: f64) -> Vec<u8> {
        self.runtime.advance(dt);
        let frame = self.runtime.frame(self.view.view(SIZE), Quality::Q3);
        let image = r.render_scene(&mut self.view, &self.runtime, &frame, SIZE, Quality::Q3, &[]).unwrap();
        let (mut target, mut pixels) = (r.create_target(SIZE.0, SIZE.1).unwrap(), Vec::new());
        let layer = Layer {
            image: Some(image),
            fit: Fit::Stretch,
            background: [0; 3],
            opacity: 1.0,
            uv: None,
            relief: None,
            flip: [false; 2],
            grade: None,
        };
        r.render_layers(&mut target, &[layer], &mut pixels).unwrap();
        pixels
    }

    fn census(&self, r: &Renderer) -> Census {
        r.census(&self.view).unwrap()[0].1
    }
}

fn at(pixels: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = ((y * SIZE.0 + x) * 4) as usize;
    [pixels[i + 2], pixels[i + 1], pixels[i]]
}

fn layer(blend: &str) -> Value {
    json!([{ "id": "p", "type": "particles", "system": "dots", "blend": blend }])
}

fn fountain() -> Value {
    json!({ "dots": {
        "capacity": 1500, "seed": 11, "prewarm": 1.0,
        "emitters": [{ "shape": { "circle": { "center": [80, 70], "radius": 6 } }, "rate": 400 }, { "shape": "edge", "burst": { "count": 60, "every": 0.5 } }],
        "init": { "life": [0.4, 1.6], "speed": [30, 90], "angle": [240, 300], "size": [2, 5], "color": ["#ffd27a", "#ff7a3d"] },
        "forces": [{ "gravity": [0, 40] }, { "turbulence": { "strength": 25, "scale": 0.05 } }, { "attractor": { "target": [80, 20], "strength": 1500 } }, { "drag": 0.3 }],
        "collide": [{ "bounds": "canvas", "bounce": 0.4, "friction": 0.1 }, { "bounds": { "circle": { "center": [40, 40], "radius": 10 } }, "kill": true }]
    } })
}

#[test]
fn gpu_and_cpu_paths_agree_on_statistics() {
    let Some(r) = renderer() else { return };
    let Some(mut gpu) = Run::new(r, fountain(), layer("add"), true) else { return };
    let mut cpu = Run::new(r, fountain(), layer("add"), false).unwrap();
    for _ in 0..45 {
        gpu.frame(r, 1.0 / 30.0);
        cpu.frame(r, 1.0 / 30.0);
    }
    let (g, c) = (gpu.census(r), cpu.census(r));
    assert!(c.alive > 300, "{c:?}");
    assert!(g.alive.abs_diff(c.alive) <= c.alive / 50, "alive {g:?} {c:?}");
    for i in 0..2 {
        assert!((g.mean[i] - c.mean[i]).abs() < 2.0, "mean {g:?} {c:?}");
        assert!((g.min[i] - c.min[i]).abs() < 4.0 && (g.max[i] - c.max[i]).abs() < 4.0, "box {g:?} {c:?}");
    }
    assert!(gpu.view.stats().steps == 2 && gpu.view.stats().particles == 1500);
}

#[test]
fn particles_stop_while_the_scene_is_paused() {
    let Some(r) = renderer() else { return };
    for gpu in [false, true] {
        let Some(mut run) = Run::new(r, fountain(), layer("normal"), gpu) else { return };
        for _ in 0..10 {
            run.frame(r, 1.0 / 30.0);
        }
        let (moving, census) = (run.frame(r, 1.0 / 30.0), run.census(r));
        assert!(run.view.stats().steps > 0);
        let paused = run.frame(r, 0.0);
        assert_eq!((run.view.stats().steps, run.census(r)), (0, census), "gpu {gpu}");
        assert_eq!(paused, moving);
        assert_eq!(run.view.stats().created, 0, "steady frames create nothing");
        assert_ne!(run.frame(r, 1.0 / 30.0), paused);
    }
}

#[test]
fn a_trimmed_view_starts_its_particles_again() {
    let Some(r) = renderer() else { return };
    for gpu in [false, true] {
        let Some(mut run) = Run::new(r, fountain(), layer("add"), gpu) else { return };
        run.frame(r, 1.0 / 30.0);
        let warm = run.census(r).alive;
        run.view.trim();
        run.runtime.restart_particles();
        run.frame(r, 0.0);
        assert_eq!((run.view.stats().steps, run.view.stats().particles), (15, 1500), "gpu {gpu}");
        assert!(run.census(r).alive.abs_diff(warm) <= warm / 50, "{warm} {:?}", run.census(r));
    }
}

#[test]
fn particles_match_their_golden_pixels() {
    let Some(r) = renderer() else { return };
    let systems = json!({ "dots": {
        "capacity": 8,
        "emitters": [
            { "shape": { "point": [40, 40] }, "burst": { "count": 1 } },
            { "shape": { "point": [120, 40] }, "burst": { "count": 2 } }
        ],
        "init": { "life": [10, 10], "speed": [0, 0], "size": [24, 24], "color": ["#ff0000", "#ff0000"] },
        "render": { "color_over_life": [[0, "#ffffff"], [1, "#00000000"]] }
    } });
    for gpu in [false, true] {
        let Some(mut run) = Run::new(r, systems.clone(), layer("add"), gpu) else { return };
        let pixels = run.frame(r, 0.0);
        let golden =
            [((40, 40), [255, 0, 0]), ((120, 40), [255, 0, 0]), ((80, 40), [0; 3]), ((40, 60), [0; 3])];
        for ((x, y), expected) in golden {
            let found = at(&pixels, x, y);
            assert!(
                found.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                "{x},{y}: {found:?}, gpu {gpu}"
            );
        }
        let edge = at(&pixels, 40 + 10, 40)[0];
        assert!(edge > 20 && edge < 250, "soft edge {edge}, gpu {gpu}");
        let mut half = Run::new(
            r,
            systems.clone(),
            json!([{ "id": "p", "type": "particles", "system": "dots", "opacity": 0.5 }]),
            gpu,
        )
        .unwrap();
        let pixels = half.frame(r, 0.0);
        assert!(at(&pixels, 40, 40)[0].abs_diff(188) <= 3, "{:?} gpu {gpu}", at(&pixels, 40, 40));
        let textured = json!({ "dots": { "capacity": 2, "render": { "texture": "pair" },
            "emitters": [{ "shape": { "point": [80, 40] }, "burst": { "count": 1 } }],
            "init": { "life": [10, 10], "speed": [0, 0], "size": [40, 40], "frame": [1, 1] } } });
        let pixels = Run::new(r, textured, layer("normal"), gpu).unwrap().frame(r, 0.0);
        assert_eq!((at(&pixels, 80, 40), at(&pixels, 80, 10)), ([0, 0, 255], [0; 3]), "gpu {gpu}");
    }
}
