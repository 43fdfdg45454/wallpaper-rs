use serde_json::json;

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
    let (frames, mut passes) = wprs_render::profiled_passes();
    passes.sort_by(|a, b| b.1.total_cmp(&a.1));
    let total: f64 = passes.iter().map(|p| p.1).sum();
    report["pass_ms"] = json!({ "frames": frames, "total": total, "passes": passes });
    let head =
        format!("passes       {total:.3} ms a frame in passes, from GPU timestamps of {frames} frames\n");
    passes.iter().fold(head, |all, (label, ms)| all + &format!("  {ms:8.4} ms  {label}\n"))
}
