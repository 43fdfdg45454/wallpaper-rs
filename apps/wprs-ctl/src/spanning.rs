use crate::told::{fill, told};
use anyhow::{Context, Result, anyhow, ensure};
use image::{Rgba, RgbaImage, imageops};
use std::collections::BTreeMap;
use std::path::{Component, Path};
use wprs_core::layout::Monitor;
use wprs_core::spread::{bounds, checked, rects, simulated};
use wprs_editor::Project;
use wprs_editor::actions::visible;
use wprs_render::{Renderer, SceneAssets};

pub const EDGE: u32 = 1600;
const FRAME: Rgba<u8> = Rgba([24, 24, 24, 255]);

async fn mine() -> Result<Vec<Monitor>> {
    let client = wprs_client::Client::connect().await?;
    let state: wprs_core::model::State = wprs_client::parse(&client.daemon.get_state().await?)?;
    let placed = state.outputs.into_iter().filter_map(|(id, o)| Some(Monitor { id, mm: o.mm? }));
    Ok(placed.collect())
}

fn monitors(layout: &str, bezel: f64) -> Result<Vec<Monitor>> {
    if layout != "mine" {
        return simulated(layout, bezel)
            .ok_or_else(|| anyhow!("{layout} is not dual, triple, vertical or mine"));
    }
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    let placed = runtime.block_on(mine())?;
    ensure!(!placed.is_empty(), "the desktop has not told where its screens are yet");
    checked(placed).map_err(|e| anyhow!(e))
}

fn inside(path: &str) -> bool {
    Path::new(path).components().all(|c| matches!(c, Component::Normal(_)))
}

fn poster(dir: &Path, edge: u32, time: Option<f64>) -> Result<RgbaImage> {
    let project = Project::open(dir)?;
    let scene = wprs_scene::Scene::from_json(&visible(&project.doc).to_string())?;
    let read = |path: &str| match inside(path) {
        true => std::fs::read(project.dir.join(path)).map_err(|e| format!("{path}: {e}")),
        false => Err(format!("{path} is outside the scene folder")),
    };
    let assets = SceneAssets::read(&scene, read).map_err(|e| anyhow!(e))?;
    let properties: BTreeMap<String, wprs_format::Property> =
        serde_json::from_value(project.doc["manifest"]["properties"].clone()).unwrap_or_default();
    let props = wprs_scene::effective(&properties, &[]);
    let renderer = Renderer::new()?;
    let time = time.unwrap_or(f64::from(scene.clock.poster_time));
    Ok(renderer.still(&scene, &assets, &props, (0.0, "en"), (edge, time))?)
}

pub fn compose(canvas: &RgbaImage, monitors: &[Monitor], edge: u32) -> RgbaImage {
    let [x0, y0, w, h] = bounds(monitors);
    let scale = f64::from(edge) / w.max(h);
    let side = |v: f64| (v * scale).round().max(1.0) as u32;
    let mut out = RgbaImage::from_pixel(side(w), side(h), FRAME);
    let size = [canvas.width(), canvas.height()];
    for ((_, [x, y, cw, ch]), m) in rects(monitors, size).into_iter().zip(monitors) {
        let (px, py) = (x.max(0.0) as u32, y.max(0.0) as u32);
        let crop = imageops::crop_imm(canvas, px, py, cw.round().max(1.0) as u32, ch.round().max(1.0) as u32);
        let part = imageops::resize(&crop.to_image(), side(m.mm[2]), side(m.mm[3]), imageops::Triangle);
        let at = |v: f64, o: f64| ((v - o) * scale).round() as i64;
        imageops::overlay(&mut out, &part, at(m.mm[0], x0), at(m.mm[1], y0));
    }
    out
}

pub fn preview(dir: &Path, layout: &str, out: &Path, time: Option<f64>) -> Result<()> {
    let bezel = wprs_core::Config::default_path().and_then(|p| wprs_core::Config::load(&p));
    let monitors = monitors(layout, bezel.map_or(0.0, |c| c.layout.bezel_mm))?;
    let canvas = poster(dir, EDGE * 2, time)?;
    let image = compose(&canvas, &monitors, EDGE);
    wprs_render::save(&image, out, 95).with_context(|| format!("cannot write {}", out.display()))?;
    println!("{}", out.display());
    Ok(())
}

pub(crate) fn second(text: &str) -> Result<f64, String> {
    let most = f64::from(wprs_scene::MAX_POSTER_TIME);
    let time: f64 = text.parse().map_err(|_| fill(told("%1 is not a number of seconds"), &[&text]))?;
    let fits = time.is_finite() && time <= most;
    fits.then_some(time)
        .ok_or_else(|| fill(told("the time is a finite number of seconds up to %1, not %2"), &[&most, &text]))
}
