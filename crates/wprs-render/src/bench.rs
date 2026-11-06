use crate::{GpuImage, Layer, RenderError, Renderer, SceneAssets, Stamping};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;
use wprs_scene::{Quality, Runtime, Tier};

mod ceiling;
mod flight;
mod frames;
mod paced;

pub use ceiling::{Ceiling, ceiling};

pub const WARMUP: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Level {
    Fixed(Quality),
    Fitted,
    Auto,
}

#[derive(Debug, Clone)]
pub struct Bench {
    pub frames: usize,
    pub size: (u32, u32),
    pub fps: u32,
    pub level: Level,
    pub tier: Option<Tier>,
    pub screens: usize,
    pub passes: bool,
    pub shot: Option<PathBuf>,
    pub paced: Option<Duration>,
    pub stamps: Stamping,
    pub overlap: bool,
}

impl Default for Bench {
    fn default() -> Bench {
        Bench {
            frames: 300,
            size: (1920, 1080),
            fps: 60,
            level: Level::Fitted,
            tier: None,
            screens: 1,
            passes: false,
            shot: None,
            paced: None,
            stamps: Stamping::All,
            overlap: false,
        }
    }
}

pub fn scene_bench(
    r: &Renderer,
    mut runtime: Runtime,
    assets: SceneAssets,
    bench: &Bench,
) -> Result<Value, RenderError> {
    let tier = bench.tier.unwrap_or_else(|| r.tier());
    let magnify = runtime.scene.magnify(bench.size);
    runtime.set_magnify(magnify);
    assets.sheets.iter().for_each(|(id, d)| runtime.set_sheet(id, d.clone()));
    let fitted = Quality::fitting(|q| runtime.scene.cost_at(q, magnify), tier, tier.quality(false));
    let quality = match bench.level {
        Level::Fixed(q) => q,
        Level::Fitted | Level::Auto => fitted,
    };
    let mut run = frames::Run::new(r, runtime, assets, bench, (tier, quality))?;
    run.view.profile_particles(bench.paced.is_none() && !bench.overlap);
    run.view.time_passes(bench.passes);
    run.view.stamping(bench.stamps);
    crate::reset_profiled_passes();
    let mut report = match bench.paced {
        Some(length) => paced::paced(&mut run, length)?,
        None => run.unpaced()?,
    };
    let stats = run.view.stats();
    let alive: u32 = r.census(&run.view)?.iter().map(|(_, c)| c.alive).sum();
    let gpu_mib = r.gpu_memory().map_or(-1.0, |b| b as f64 / 1048576.0);
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let rss =
        status.lines().find_map(|l| l.strip_prefix("VmRSS:")).and_then(|v| v.trim().strip_suffix(" kB"));
    let rss_mib = rss.and_then(|kb| kb.trim().parse::<f64>().ok()).map_or(-1.0, |kb| kb / 1024.0);
    let engine = run.view.gpu_particles().map_or("none", |gpu| if gpu { "gpu" } else { "cpu" });
    let costs = [Quality::Q0, Quality::Q1, Quality::Q2, Quality::Q3].map(|q| run.runtime.cost(q));
    let more = json!({
        "adapter": r.adapter(), "tier": tier, "quality": run.quality, "size": [bench.size.0, bench.size.1],
        "render_size": [run.inner.0, run.inner.1], "cost_units": run.runtime.cost(run.quality),
        "costs": costs, "passes": stats.passes, "draws": stats.draws, "textures": stats.textures,
        "cached": stats.reused, "screens": bench.screens, "load_ms": run.load_ms, "gpu_mib": gpu_mib,
        "rss_mib": rss_mib, "left_out": run.view.left_out(),
    });
    let particles = json!({ "engine": engine, "slots": stats.particles, "alive": alive });
    more.as_object().into_iter().flatten().for_each(|(k, v)| report[k] = v.clone());
    particles.as_object().into_iter().flatten().for_each(|(k, v)| report["particles"][k] = v.clone());
    if let Some((pilot, _)) = &run.pilot {
        let levels: Vec<Value> = run.levels.iter().map(|(t, q)| json!([t, q])).collect();
        let (frame_ms, period_ms) = (pilot.frame_ms(), 1000.0 / bench.fps.max(1) as f32);
        let (step, still) = (pilot.step(), pilot.stalled());
        report["auto"] = json!({ "start": quality, "levels": levels, "frame_ms": frame_ms,
            "period_ms": period_ms, "step": step, "still": still });
    }
    if bench.passes {
        let (frames, mut passes) = crate::profiled_passes(None);
        passes.sort_by(|a, b| b.1.total_cmp(&a.1));
        let total: f64 = passes.iter().map(|p| p.1).sum();
        report["pass_ms"] = json!({ "frames": frames, "total": total, "passes": passes });
    }
    Ok(report)
}

pub fn save_frame(
    r: &Renderer,
    image: &GpuImage,
    (w, h): (u32, u32),
    file: &Path,
) -> Result<(), RenderError> {
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
    let wrong = || RenderError::Readback("the frame has the wrong size".into());
    let saved = image::RgbaImage::from_raw(w, h, pixels).ok_or_else(wrong)?.save(file);
    saved.map_err(|e| RenderError::Readback(format!("cannot write {}: {e}", file.display())))
}

pub fn spread(mut samples: Vec<f64>) -> Value {
    samples.sort_by(f64::total_cmp);
    let at =
        |q: f64| samples.get(((samples.len() as f64 - 1.0) * q).round() as usize).copied().unwrap_or(0.0);
    let mean = samples.iter().sum::<f64>() / samples.len().max(1) as f64;
    json!({ "mean": mean, "p50": at(0.5), "p95": at(0.95), "max": at(1.0) })
}
