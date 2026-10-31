use image::codecs::jpeg::JpegEncoder;
use image::{GrayImage, Luma, Rgb, RgbImage};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wprs_core::Fit;
use wprs_format::{Limits, Rating, SCHEMA, build, features, inspect, open};
use wprs_render::{Layer, Renderer, SceneAssets};
use wprs_scene::{PointerState, Quality, Runtime, Wake, effective, playable};

const SIZE: (u32, u32) = (480, 270);
const STILL: &[&str] = &["dusk", "pond", "pulse"];
const SOUNDS: &[&str] = &["pond", "snowfall"];
const DEPTHS: [(&str, f32, f32); 4] =
    [("far", 0.3, 0.3), ("lake", 0.4, 0.65), ("left", 0.85, 0.85), ("right", 0.95, 0.95)];
const SOFT: f32 = 4.0;

fn scenes() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenes");
    let mut dirs: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("scene.json").is_file())
        .collect();
    dirs.sort();
    dirs
}

fn renderer() -> Option<&'static Renderer> {
    match Renderer::shared() {
        Ok(r) => Some(r),
        Err(e) if std::env::var_os("WPRS_REQUIRE_GPU").is_none() => {
            eprintln!("skipping the GPU part: {e}");
            None
        }
        Err(e) => panic!("GPU required but unavailable: {e}"),
    }
}

fn render(r: &Renderer, runtime: &mut Runtime, assets: &SceneAssets) -> (RgbImage, Wake, u32) {
    let mut view = r.scene_view(&runtime.scene, assets, runtime.props()).unwrap();
    let (mut target, mut pixels, mut wake, mut created) =
        (r.create_target(SIZE.0, SIZE.1).unwrap(), Vec::new(), Wake::Frame, 0);
    for i in 0..120 {
        runtime.advance(1.0 / 12.0);
        let frame = runtime.frame(view.view(SIZE), Quality::Q3);
        let image = r.render_scene(&mut view, runtime, &frame, SIZE, Quality::Q3, &[]).unwrap();
        let layer = Layer {
            image: Some(image),
            fit: Fit::Stretch,
            background: [0; 3],
            opacity: 1.0,
            uv: None,
            relief: None,
        };
        r.render_layers(&mut target, &[layer], &mut pixels).unwrap();
        (wake, created) = (frame.wake, created + if i >= 70 { view.stats().created } else { 0 });
    }
    let at = |x: u32, y: u32, c: usize| pixels[((y * SIZE.0 + x) * 4) as usize + 2 - c];
    (RgbImage::from_fn(SIZE.0, SIZE.1, |x, y| Rgb([at(x, y, 0), at(x, y, 1), at(x, y, 2)])), wake, created)
}

#[test]
fn example_scenes_pack_small_render_and_rest() {
    let (dir, r) = (tempfile::tempdir().unwrap(), renderer());
    let mut total = 0;
    for source in scenes() {
        let name = source.file_name().unwrap().to_string_lossy().into_owned();
        let out = dir.path().join(format!("{name}.wpak"));
        let (declared, facts) = inspect(&source).unwrap();
        assert_eq!(declared.features(), facts.features.iter().cloned().collect::<Vec<_>>(), "{name}");
        let warned = features::warnings(&declared, &facts);
        assert!(warned.iter().all(|w| w.contains("older integrated and phone GPUs")), "{name}: {warned:?}");
        build(&source, &out, &Limits::default()).unwrap_or_else(|e| panic!("{name}: {e}"));
        total += std::fs::metadata(&out).unwrap().len();
        let mut package = open(&out, &Limits::default()).unwrap();
        let manifest = package.manifest.clone();
        assert_eq!(
            (manifest.schema.as_str(), manifest.license.as_str(), manifest.thumbnail().map(String::as_str)),
            (SCHEMA, "CC0-1.0", Some("preview.jpg")),
            "{name}"
        );
        let titles = ["en", "es"].map(|l| manifest.title_in(&[l.into()]));
        assert!(titles[0] != titles[1] && manifest.locales().len() == 2, "{name} speaks English and Spanish");
        let flashing = manifest.photosensitive == Some(name == "pulse");
        assert!(flashing && manifest.content.as_ref().is_some_and(|c| c.rating == Rating::General), "{name}");
        let scene = Arc::new(package.scene().unwrap());
        assert_eq!(scene.sounds().count(), usize::from(SOUNDS.contains(&name.as_str())), "{name}");
        let pictures: Vec<_> = scene.assets.iter().filter(|(_, a)| !playable(&a.path)).collect();
        assert!(pictures.iter().all(|(_, a)| a.linear), "{name} draws its pictures itself");
        let mut assets = SceneAssets::default();
        for (id, asset) in pictures {
            let data = image::load_from_memory(&package.read_file(&asset.path).unwrap()).unwrap();
            assets.images.insert(id.clone(), wprs_render::Pixels::new(data.into_rgba8()));
        }
        let Some(r) = &r else { continue };
        let fresh = || Runtime::new(scene.clone(), effective(&manifest.properties, &[]));
        let (shot, wake, created) = render(r, &mut fresh(), &assets);
        assert_eq!(created, 0, "{name} created GPU resources in a steady frame");
        assert_eq!(wake == Wake::Idle, STILL.contains(&name.as_str()), "{name} ends with {wake:?}");
        let count = f64::from(SIZE.0 * SIZE.1);
        let mean = shot.pixels().map(|p| f64::from(p.0[1])).sum::<f64>() / count;
        let spread = shot.pixels().map(|p| (f64::from(p.0[1]) - mean).powi(2)).sum::<f64>() / count;
        assert!(spread.sqrt() > 8.0, "{name} looks flat");
        let mut runtime = fresh();
        if runtime.uses().cursor {
            let at = [1.0, 0.5];
            runtime.set_pointer(PointerState {
                pos: Some(at),
                smooth: at,
                inside: true,
                presence: 1.0,
                ..Default::default()
            });
            let moved = render(r, &mut runtime, &assets).0;
            let changed = moved.pixels().zip(shot.pixels()).filter(|(a, b)| a != b).count();
            assert!(changed > (SIZE.0 * SIZE.1 / 10) as usize, "{name} hardly follows the pointer");
        }
        let preview = source.join("preview.jpg");
        if std::env::var_os("WPRS_BLESS").is_some() {
            let file = std::fs::File::create(&preview).unwrap();
            JpegEncoder::new_with_quality(file, 82).encode_image(&shot).unwrap();
        }
        assert_eq!(image::image_dimensions(&preview).unwrap(), SIZE, "{name}");
    }
    assert!(total < 400 << 10, "the example scenes take {total} bytes");
}

fn depth_of(scene: &Value) -> GrayImage {
    let f = |v: &Value| v.as_f64().unwrap() as f32;
    let layers = scene["layers"].as_array().unwrap();
    let shapes: Vec<_> = DEPTHS
        .iter()
        .map(|&(id, top, bottom)| {
            let layer = layers.iter().find(|l| l["id"] == id).unwrap();
            let round = layer["shape"]["ellipse"].is_object();
            let (at, size) = (
                &layer["transform"]["position"],
                &layer["shape"][if round { "ellipse" } else { "rect" }]["size"],
            );
            ([f(&at[0]), f(&at[1])], [f(&size[0]) / 2.0, f(&size[1]) / 2.0], round, top, bottom)
        })
        .collect();
    let [w, h] = [0, 1].map(|i| scene["canvas"]["size"][i].as_u64().unwrap() as u32 / 2);
    GrayImage::from_fn(w, h, |x, y| {
        let p = [x as f32 * 2.0 + 1.0, y as f32 * 2.0 + 1.0];
        let depth = shapes.iter().fold(0.0, |depth, &([cx, cy], [a, b], round, top, bottom)| {
            let (dx, dy) = (p[0] - cx, p[1] - cy);
            let edge = match round {
                true => {
                    let (gx, gy) = (dx / (a * a), dy / (b * b));
                    (dx * gx + dy * gy - 1.0) / (2.0 * (gx * gx + gy * gy).sqrt()).max(1e-6)
                }
                false => (dx.abs() - a).max(dy.abs() - b),
            };
            let cover = (0.5 - edge / (2.0 * SOFT)).clamp(0.0, 1.0);
            let value = top + (bottom - top) * ((dy + b) / (2.0 * b)).clamp(0.0, 1.0);
            depth + (value - depth) * cover
        });
        Luma([(depth * 255.0).round() as u8])
    })
}

#[test]
fn the_dusk_depth_map_follows_its_hills() {
    let dir = scenes().into_iter().find(|d| d.ends_with("dusk")).unwrap();
    let scene: Value = serde_json::from_slice(&std::fs::read(dir.join("scene.json")).unwrap()).unwrap();
    let (path, depth) = (dir.join("depth.png"), depth_of(&scene));
    if std::env::var_os("WPRS_BLESS").is_some() {
        depth.save(&path).unwrap();
    }
    let same = image::open(&path).unwrap().into_luma8() == depth;
    assert!(same, "depth.png does not match the hills of scene.json; run this test with WPRS_BLESS=1");
}
