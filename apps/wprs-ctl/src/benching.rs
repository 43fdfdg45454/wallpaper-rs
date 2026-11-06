use anyhow::{Context, Result};
use serde_json::json;
use std::path::Path;
use wprs_format::{Limits, open};
use wprs_render::{GpuImage, Layer, Renderer, SceneAssets};

pub(crate) fn load(path: &Path) -> Result<(wprs_scene::Scene, wprs_scene::Values, SceneAssets)> {
    let mut package =
        open(path, &Limits::default()).with_context(|| format!("cannot open {}", path.display()))?;
    let scene = package.scene()?;
    let mut assets = SceneAssets::read(&scene, |path| package.read_file(path).map_err(|e| e.to_string()))
        .map_err(anyhow::Error::msg)?;
    let file = path.to_owned();
    assets.reloadable(&scene, move || {
        let mut package = open(&file, &Limits::default()).ok()?;
        Some(move |path: &str| package.read_file(path).map_err(|e| e.to_string()))
    });
    let values = wprs_scene::effective(&package.manifest.properties, &[]);
    Ok((scene, values, assets))
}

pub(crate) fn shot(r: &Renderer, image: &GpuImage, (w, h): (u32, u32), file: &Path) -> Result<()> {
    let (mut target, mut pixels) = (r.create_target(w, h)?, Vec::new());
    let layer = Layer {
        image: Some(image),
        fit: wprs_core::Fit::Stretch,
        background: [0; 3],
        opacity: 1.0,
        uv: None,
        relief: None,
        flip: [false; 2],
        grade: None,
    };
    r.render_layers(&mut target, &[layer], &mut pixels)?;
    pixels.as_chunks_mut::<4>().0.iter_mut().for_each(|p| p.swap(0, 2));
    image::RgbaImage::from_raw(w, h, pixels)
        .context("the frame has the wrong size")?
        .save(file)
        .with_context(|| format!("cannot write {}", file.display()))
}

pub(crate) fn spread(mut samples: Vec<f64>) -> serde_json::Value {
    samples.sort_by(f64::total_cmp);
    let at =
        |q: f64| samples.get(((samples.len() as f64 - 1.0) * q).round() as usize).copied().unwrap_or(0.0);
    let mean = samples.iter().sum::<f64>() / samples.len().max(1) as f64;
    json!({ "mean": mean, "p50": at(0.5), "p95": at(0.95), "max": at(1.0) })
}

pub(crate) fn ms_of(value: &serde_json::Value, unit: &str) -> String {
    let at = |key: &str| value[key].as_f64().unwrap_or(0.0);
    format!("{:.3} {unit} (p95 {:.3}, max {:.3})", at("mean"), at("p95"), at("max"))
}

pub(crate) fn passes(report: &mut serde_json::Value) -> String {
    let (frames, mut passes) = wprs_render::profiled_passes(None);
    passes.sort_by(|a, b| b.1.total_cmp(&a.1));
    let total: f64 = passes.iter().map(|p| p.1).sum();
    report["pass_ms"] = json!({ "frames": frames, "total": total, "passes": passes });
    let head =
        format!("passes       {total:.3} ms a frame in passes, from GPU timestamps of {frames} frames\n");
    passes.iter().fold(head, |all, (label, ms)| all + &format!("  {ms:8.4} ms  {label}\n"))
}
