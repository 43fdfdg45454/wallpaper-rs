use anyhow::{Context, Result};
use serde_json::Value;
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

pub fn steered(auto: &Value) -> String {
    let half = |pace: &Value| if pace.as_u64().unwrap_or(1) > 1 { " at half pace" } else { "" };
    let mut out = String::new();
    if let Some(levels) = auto["levels"].as_array() {
        let at = |l: &Value| format!("{}{} at {:.1} s", l[1], half(&l[2]), l[0].as_f64().unwrap_or(0.0));
        out += &format!("auto         {}\n", levels.iter().map(at).collect::<Vec<_>>().join(", "));
    }
    if let Some(frame) = auto["frame_ms"].as_f64() {
        let (period, step) = (auto["period_ms"].as_f64().unwrap_or(0.0), auto["step"].as_str().unwrap_or(""));
        let (fps, pace) =
            (auto["pace_fps"].as_u64().unwrap_or(0), half(&auto["pace"]).trim_start_matches(" at "));
        let pace = if pace.is_empty() { "full pace" } else { pace };
        let measured = format!("{frame:.2} ms a frame of {period:.2} ms at {fps} fps ({pace})");
        out += &format!("measured     {measured}, last decision {step}\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::steered;
    use serde_json::json;

    #[test]
    fn the_automatic_level_prints_its_pace() {
        let auto = json!({ "levels": [[0.0, "q3", 1], [2.2, "q3", 2], [9.0, "q2", 2]], "frame_ms": 12.5,
            "period_ms": 33.33, "step": "halved", "pace": 2, "pace_fps": 30 });
        let text = steered(&auto);
        assert!(
            text.contains(
                "auto         \"q3\" at 0.0 s, \"q3\" at half pace at 2.2 s, \"q2\" at half pace at 9.0 s"
            ),
            "{text}"
        );
        assert!(
            text.contains("12.50 ms a frame of 33.33 ms at 30 fps (half pace), last decision halved"),
            "{text}"
        );
        let full = steered(
            &json!({ "frame_ms": 5.0, "period_ms": 16.67, "step": "steady", "pace": 1, "pace_fps": 60 }),
        );
        assert!(full.contains("at 60 fps (full pace), last decision steady"), "{full}");
    }
}
