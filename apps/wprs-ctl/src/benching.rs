use anyhow::{Context, Result};
use std::path::Path;
use wprs_format::{Limits, open};
use wprs_render::SceneAssets;

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

pub(crate) fn ms_of(value: &serde_json::Value, unit: &str) -> String {
    let at = |key: &str| value[key].as_f64().unwrap_or(0.0);
    format!("{:.3} {unit} (p95 {:.3}, max {:.3})", at("mean"), at("p95"), at("max"))
}

pub(crate) fn passes(report: &serde_json::Value) -> String {
    let timed = &report["pass_ms"];
    let Some(passes) = timed["passes"].as_array() else { return String::new() };
    let (frames, total) = (timed["frames"].as_u64().unwrap_or(0), timed["total"].as_f64().unwrap_or(0.0));
    let head =
        format!("passes       {total:.3} ms a frame in passes, from GPU timestamps of {frames} frames\n");
    passes.iter().fold(head, |all, pass| {
        let (label, ms) = (pass[0].as_str().unwrap_or_default(), pass[1].as_f64().unwrap_or(0.0));
        all + &format!("  {ms:8.4} ms  {label}\n")
    })
}
