use crate::editing::dimensions;
use anyhow::{Context, Result};
use clap::Subcommand;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wprs_format::{Limits, open};
use wprs_render::{Renderer, SceneAssets};
use wprs_scene::{Quality, Runtime, Spare, Wake, effective};

const WARMUP: usize = 10;

#[derive(Subcommand, Debug)]
pub enum SceneCommand {
    #[command(about = "Render a scene package offscreen and report the cost of each frame; needs no daemon")]
    Bench {
        path: PathBuf,
        #[arg(long, default_value_t = 300, help = "Number of frames to render")]
        frames: usize,
        #[arg(long, default_value = "1920x1080", value_name = "WxH", help = "Output size")]
        size: String,
        #[arg(long, default_value_t = 60, help = "Frames per second of the simulated clock")]
        fps: u32,
        #[arg(long, value_parser = ["q0", "q1", "q2", "q3"], help = "Quality level; picked like the daemon when omitted")]
        quality: Option<String>,
        #[arg(long, default_value_t = 1, help = "Number of screens that show the scene at once")]
        screens: usize,
        #[arg(long, help = "Print the report as JSON")]
        json: bool,
    },
    #[command(flatten)]
    Author(crate::author::Author),
}

fn load(path: &Path) -> Result<(wprs_scene::Scene, wprs_scene::Values, SceneAssets)> {
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
    let values = effective(&package.manifest.properties, &[]);
    Ok((scene, values, assets))
}

fn spread(mut samples: Vec<f64>) -> serde_json::Value {
    samples.sort_by(f64::total_cmp);
    let at =
        |q: f64| samples.get(((samples.len() as f64 - 1.0) * q).round() as usize).copied().unwrap_or(0.0);
    let mean = samples.iter().sum::<f64>() / samples.len().max(1) as f64;
    json!({ "mean": mean, "p50": at(0.5), "p95": at(0.95), "max": at(1.0) })
}

fn ms_of(value: &serde_json::Value, unit: &str) -> String {
    let at = |key: &str| value[key].as_f64().unwrap_or(0.0);
    format!("{:.3} {unit} (p95 {:.3}, max {:.3})", at("mean"), at("p95"), at("max"))
}

pub fn run(command: SceneCommand) -> Result<()> {
    let (path, frames, size, fps, quality, screens, json) = match command {
        SceneCommand::Bench { path, frames, size, fps, quality, screens, json } => {
            (path, frames, size, fps, quality, screens.clamp(1, 16), json)
        }
        SceneCommand::Author(command) => return crate::authoring::run(command),
    };
    let size = dimensions(&size)?;
    let (scene, values, assets) = load(&path)?;
    let r = Renderer::new()?;
    let calibrated = r.calibrate(wprs_core::gpu_tier().as_deref());
    let (tier, unit_ms) = (r.tier(), calibrated.unit_ms);
    let quality = match quality.as_deref() {
        Some(q) => serde_json::from_value(json!(q))?,
        None => Quality::fit(&scene, tier, tier.quality(false)),
    };
    let cost = scene.cost(quality);
    let mut runtime = Runtime::new(Arc::new(scene), values);
    runtime.set_quality(quality);
    assets.sheets.iter().for_each(|(id, d)| runtime.set_sheet(id, d.clone()));
    let start = Instant::now();
    let mut others = Vec::new();
    for _ in 1..screens {
        others.push(r.scene_view(&runtime.scene, &assets, runtime.props())?);
    }
    let mut view = r.scene_view(&runtime.scene, &assets, runtime.props())?;
    let mut inner = view.inner(&r, size, quality);
    let load_ms = start.elapsed().as_secs_f64() * 1e3;
    drop(assets);
    view.profile_particles(true);
    let (mut eval, mut render, mut needed, mut created) = (Vec::new(), Vec::new(), 0, 0);
    let (mut sim, mut steps, mut compiled, mut gpu, mut sim_gpu) = (Vec::new(), 0, 0, Vec::new(), Vec::new());
    let (mut wake, mut spare) = (Wake::Frame, Spare::default());
    let mut since = Duration::ZERO;
    let step = Duration::from_secs(1) / fps.max(1);
    for i in 0..frames.max(1) {
        if i > 0 {
            runtime.advance(step.as_secs_f64());
            since += step;
        }
        let due = match wake {
            Wake::Frame => true,
            Wake::At(s) => since.as_secs_f64() >= s,
            Wake::Idle => false,
        };
        needed += usize::from(due);
        since = if due { Duration::ZERO } else { since };
        inner = view.inner(&r, size, quality);
        let start = Instant::now();
        let frame = runtime.frame_in(std::mem::take(&mut spare), view.view(inner), quality);
        eval.push(start.elapsed().as_secs_f64() * 1e6);
        let start = Instant::now();
        for other in &mut others {
            r.render_scene(other, &runtime, &frame, inner, quality, &[])?;
        }
        r.render_scene(&mut view, &runtime, &frame, inner, quality, &[])?;
        r.finish()?;
        render.push(start.elapsed().as_secs_f64() * 1e3);
        wake = frame.wake;
        spare = frame.spare();
        if i >= WARMUP {
            let s = view.stats();
            (created, steps, compiled) = (created + s.created, steps + s.steps, compiled + s.compiled);
            sim.push(f64::from(s.sim_us) / 1e3);
            gpu.push(f64::from(s.gpu_us) / 1e3);
            sim_gpu.push(f64::from(s.sim_gpu_us) / 1e3);
        }
    }
    let steady = frames.saturating_sub(WARMUP).max(1);
    let stats = view.stats();
    let alive: u32 = r.census(&view)?.iter().map(|(_, c)| c.alive).sum();
    let first_ms = render.first().copied().unwrap_or(0.0);
    let gpu_mib = r.gpu_memory().map_or(-1.0, |b| b as f64 / 1048576.0);
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let rss =
        status.lines().find_map(|l| l.strip_prefix("VmRSS:")).and_then(|v| v.trim().strip_suffix(" kB"));
    let rss_mib = rss.and_then(|kb| kb.trim().parse::<f64>().ok()).map_or(-1.0, |kb| kb / 1024.0);
    let engine = view.gpu_particles().map_or("none", |gpu| if gpu { "gpu" } else { "cpu" });
    let particles = json!({
        "engine": engine, "slots": stats.particles, "alive": alive, "steps": steps, "sim_ms": spread(sim),
        "sim_gpu_ms": spread(sim_gpu),
    });
    let report = json!({
        "path": path, "adapter": r.adapter(), "tier": tier, "unit_ms": unit_ms, "quality": quality, "size": [size.0, size.1],
        "render_size": [inner.0, inner.1], "frames": frames, "needed": needed, "cost_units": cost,
        "eval_us": spread(eval), "render_ms": spread(render), "gpu_ms": spread(gpu), "passes": stats.passes, "draws": stats.draws,
        "textures": stats.textures, "cached": stats.reused, "created_after_warmup": created,
        "compiled_after_warmup": compiled, "screens": screens, "load_ms": load_ms, "first_ms": first_ms,
        "gpu_mib": gpu_mib, "rss_mib": rss_mib,
        "particles": particles, "left_out": view.left_out(),
    });
    if json {
        println!("{report}");
        return Ok(());
    }
    let ms = |key: &str, unit: &str| ms_of(&report[key], unit);
    println!(
        "adapter      {}\ntier         {tier:?} ({}), quality {quality:?}, {}x{} rendered at {}x{}",
        r.adapter(),
        if unit_ms > 0.0 { format!("{unit_ms:.3} ms a unit") } else { "from its model".into() },
        size.0,
        size.1,
        inner.0,
        inner.1
    );
    println!("frames       {frames}, {needed} needed by the scheduler, estimated cost {cost:.2} units");
    println!("evaluation   {}\nrender       {}", ms("eval_us", "µs"), ms("render_ms", "ms"));
    println!("gpu          {}", ms("gpu_ms", "ms"));
    println!(
        "start        {load_ms:.1} ms to load, {first_ms:.1} ms for the first frame of {screens} screens"
    );
    println!("memory       {gpu_mib:.1} MiB on the GPU, {rss_mib:.1} MiB resident");
    println!(
        "last frame   {} passes, {} draws, {} pooled textures, {} cached parts",
        stats.passes, stats.draws, stats.textures, stats.reused
    );
    println!(
        "created      {created} GPU resources, {compiled} of them shaders, after {WARMUP} warm-up frames"
    );
    view.left_out().iter().for_each(|note| println!("left out     {note}"));
    if engine != "none" {
        let per_frame = f64::from(steps) / steady as f64;
        println!(
            "particles    {engine}, {} slots, {alive} alive at the end, {per_frame:.1} steps per frame",
            stats.particles
        );
        println!(
            "simulation   {}, {} of it on the GPU, after {WARMUP} warm-up frames",
            ms_of(&report["particles"]["sim_ms"], "ms"),
            ms_of(&report["particles"]["sim_gpu_ms"], "ms")
        );
    }
    Ok(())
}
