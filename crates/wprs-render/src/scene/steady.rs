use super::{Op, SceneAssets, SceneView, encode, pipes};
use crate::allocations::allocations;
use crate::{Pixels, Renderer};
use image::{Rgba, RgbaImage};
use std::sync::Arc;
use wprs_scene::{Budget, Quality, Runtime, Scene, Tier, Values};

pub(crate) const SCENE: &str = r##"{ "schema": "wallpaper-rs.scene/1",
  "canvas": { "size": [320, 180], "background": "#102030" },
  "assets": { "photo": { "path": "p.png", "mipmaps": true, "wrap": "repeat" },
              "sheet": { "path": "s.png", "atlas": [1, 4], "fps": 8 } },
  "timelines": [{ "id": "swing", "duration": 4, "loop": "repeat" }],
  "layers": [
    { "id": "back", "type": "image", "image": "photo", "size": [320, 180], "parallax": 0.1 },
    { "id": "spin", "type": "image", "image": "photo", "size": [60, 60], "color": "#ff8080", "brightness": 1.2,
      "transform": { "position": [60, 60], "rotation": { "timeline": "swing", "keys": [[0, 0], [4, 360]] } } },
    { "id": "kid", "type": "solid", "color": "#00ff00", "size": [12, 12], "parent": "spin", "transform": { "position": [40, 0] } },
    { "id": "sprite", "type": "image", "image": "sheet", "size": [40, 40], "transform": { "position": [270, 40] } },
    { "id": "glow", "type": "group", "color": "#c0ffc0", "effects": [{ "effect": "blur", "params": { "radius": 6 } }], "layers": [
      { "id": "dot", "type": "shape", "shape": { "ellipse": { "size": [50, 50] } }, "fill": "#ffcc66",
        "transform": { "position": { "keys": [[0, [100, 130]], [2, [220, 130]]], "loop": "pingpong" } } } ] },
    { "id": "over", "type": "solid", "color": "#4060ff", "size": [80, 50], "blend": "overlay",
      "transform": { "position": { "keys": [[0, [160, 90]], [2, [200, 100]]], "loop": "pingpong" } } },
    { "id": "masked", "type": "solid", "color": "#ffffff", "size": [40, 40], "mask": { "shape": { "ellipse": { "size": [1, 1] } } },
      "transform": { "position": [250, 140], "scale": { "keys": [[0, 1], [2, 1.5]], "loop": "pingpong" } } },
    { "id": "words", "type": "text", "text": "Steady", "size": 20, "color": "#ffffff", "transform": { "position": [160, 20] } },
    { "id": "post", "type": "adjustment", "effects": [{ "effect": "vignette", "params": { "intensity": 0.4 } }] }
  ] }"##;

pub(crate) fn assets() -> SceneAssets {
    let photo = RgbaImage::from_fn(256, 256, |x, y| Rgba([x as u8, y as u8, (x ^ y) as u8, 255]));
    let sheet = RgbaImage::from_fn(64, 16, |x, _| Rgba([(x / 16 * 80) as u8, 0, 200, 255]));
    let images = [("photo".to_string(), Pixels::new(photo)), ("sheet".to_string(), Pixels::new(sheet))];
    SceneAssets { images: images.into(), ..Default::default() }
}

fn frame(r: &Renderer, view: &mut SceneView, runtime: &mut Runtime) -> [u64; 2] {
    runtime.advance(1.0 / 30.0);
    let mut frame = None;
    let evaluated = allocations(|| frame = Some(runtime.frame(view.view((320, 180)), Quality::Q3)));
    let frame = frame.expect("evaluated above");
    let spent = allocations(|| {
        r.render_scene(view, runtime, &frame, (320, 180), Quality::Q3, &[]).unwrap();
    });
    r.finish().unwrap();
    [evaluated, spent]
}

fn replay(r: &Renderer, view: &mut SceneView, ops: &[Op], data: &[u8]) -> u64 {
    let SceneView { pool, caches, states, luts, target, swarms, scratch, uniform, .. } = view;
    let (canvas, output) = target.as_ref().unwrap();
    let swarms = swarms.as_ref();
    let sources =
        encode::Sources { caches, states, luts, exts: &scratch.exts, canvas, output, swarms, scissor: None };
    let pipes = r.pipes();
    let spent = allocations(|| {
        r.queue.write_buffer(uniform, 0, &data[..size_of::<super::item::FrameData>()]);
        pool.upload(&r.device, &r.queue, pipes, uniform);
        r.scoped(|| r.encode_scene(pool, &sources, ops, None)).unwrap();
    });
    r.finish().unwrap();
    spent
}

#[test]
fn steady_scene_frames_allocate_nothing_beyond_their_gpu_calls() {
    let r = match Renderer::shared() {
        Ok(r) => r,
        Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
    };
    let scene = Scene::load(SCENE, &Default::default()).unwrap();
    let mut runtime = Runtime::new(Arc::new(scene), Values::new());
    let mut view = r.scene_view(&runtime.scene, &assets(), runtime.props()).unwrap();
    (0..8).for_each(|_| _ = frame(r, &mut view, &mut runtime));
    let (mut ours, mut theirs, mut passes, mut evaluated) = (u64::MAX, u64::MAX, 0, 0);
    for _ in 0..6 {
        let [eval, spent] = frame(r, &mut view, &mut runtime);
        (ours, evaluated) = (ours.min(spent), evaluated.max(eval));
        passes = view.stats().passes;
        let ops = view.scratch.ops.clone();
        let slot = |op: &Op| match *op {
            Op::Draw { slot, .. } | Op::Effect { slot, .. } | Op::Particles { slot, .. } => slot as usize,
            _ => 0,
        };
        view.pool.data.resize(ops.iter().map(slot).max().unwrap_or(0) + pipes::STRIDE as usize, 0);
        theirs = theirs.min(replay(r, &mut view, &ops, &[0; 1024]));
        view.pool.data.clear();
    }
    assert!(passes >= 5 && view.stats().created == 0, "{:?}", view.stats());
    assert!(evaluated <= 3, "evaluating a steady frame made {evaluated} allocations");
    let power = Budget { heat: wprs_scene::Heat::Warm, ..Budget::new(Tier::T1, false, 0) };
    let schedule = || {
        let quality = power.quality(&runtime);
        std::hint::black_box((power.fps(&runtime, quality, 60), power.still(&runtime)));
    };
    schedule();
    assert_eq!(allocations(schedule), 0, "choosing the quality and rate of a steady frame allocated");
    assert!(ours <= theirs, "a steady frame made {ours} allocations, its GPU calls alone make {theirs}");
    let draws: Vec<Op> =
        view.scratch.ops.iter().copied().filter(|op| matches!(op, Op::Draw { .. })).collect();
    let lookups = allocations(|| {
        for op in &draws {
            let Op::Draw { entry, mode, tex, .. } = op else { continue };
            drop(r.pipes().get(&r.device, pipes::Shader::Scene(entry), *mode, view.format));
            let SceneView { pool, caches, states, luts, target, swarms, scratch, .. } = &mut view;
            let (canvas, output) = target.as_ref().unwrap();
            let exts = &scratch.exts;
            let sources = encode::Sources {
                caches,
                states,
                luts,
                exts,
                canvas,
                output,
                swarms: swarms.as_ref(),
                scissor: None,
            };
            drop(sources.group(r, pool, false, tex));
        }
    });
    assert_eq!(lookups, 0, "pipelines and bind groups of a steady frame were not all cached");
}

#[test]
fn a_loaded_scene_draws_its_first_frames_without_compiling() {
    let Ok(r) = Renderer::shared() else { return };
    let scene = Scene::load(SCENE, &Default::default()).unwrap();
    let mut runtime = Runtime::new(Arc::new(scene), Values::new());
    let mut view = r.scene_view(&runtime.scene, &assets(), runtime.props()).unwrap();
    for _ in 0..3 {
        frame(r, &mut view, &mut runtime);
        assert_eq!(view.stats().compiled, 0, "{:?}", view.stats());
    }
}
