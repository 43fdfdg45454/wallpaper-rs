use crate::editing::dimensions;
use anyhow::{Context, Result};
use clap::Subcommand;
use image::RgbaImage;
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wprs_format::{Limits, open};
use wprs_render::{Renderer, SceneAssets};
use wprs_scene::{Quality, Runtime, Tier, Wake, effective};

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
    let mut assets = SceneAssets::default();
    let mut decoded: HashMap<String, Arc<RgbaImage>> = HashMap::new();
    for (id, asset) in &scene.assets {
        let ext = Path::new(&asset.path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        match ext.as_str() {
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => {
                let image = match decoded.get(&asset.path) {
                    Some(image) => image.clone(),
                    None => Arc::new(image::load_from_memory(&package.read_file(&asset.path)?)?.into_rgba8()),
                };
                decoded.insert(asset.path.clone(), image.clone());
                let total: u64 =
                    decoded.values().map(|i| u64::from(i.width()) * u64::from(i.height()) * 4).sum();
                anyhow::ensure!(total <= 512 << 20, "the images of the scene are too large once decoded");
                assets.images.insert(id.clone(), image);
            }
            "ttf" | "otf" => drop(assets.fonts.insert(id.clone(), Arc::new(package.read_file(&asset.path)?))),
            _ => {}
        }
    }
    assets.luts.extend(package.luts(&scene)?);
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
    let (path, frames, size, fps, quality, json) = match command {
        SceneCommand::Bench { path, frames, size, fps, quality, json } => {
            (path, frames, size, fps, quality, json)
        }
        SceneCommand::Author(command) => return crate::authoring::run(command),
    };
    let size = dimensions(&size)?;
    let (scene, values, assets) = load(&path)?;
    let r = Renderer::new()?;
    let tier = Tier::detect(r.software(), r.discrete());
    let quality = match quality.as_deref() {
        Some(q) => serde_json::from_value(json!(q))?,
        None => Quality::fit(&scene, tier, tier.quality(false)),
    };
    let scale = quality.render_scale();
    let inner =
        ((size.0 as f32 * scale).round().max(1.0) as u32, (size.1 as f32 * scale).round().max(1.0) as u32);
    let cost = scene.cost(quality);
    let mut runtime = Runtime::new(Arc::new(scene), values);
    let mut view = r.scene_view(&runtime.scene, &assets)?;
    view.profile_particles(true);
    let (mut eval, mut render, mut needed, mut created) = (Vec::new(), Vec::new(), 0, 0);
    let (mut sim, mut steps, mut compiled) = (Vec::new(), 0, 0);
    let mut wake = Wake::Frame;
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
        let start = Instant::now();
        let frame = runtime.frame(view.view(inner), quality);
        eval.push(start.elapsed().as_secs_f64() * 1e6);
        let start = Instant::now();
        r.render_scene(&mut view, &runtime, &frame, inner, quality, &[])?;
        r.finish()?;
        render.push(start.elapsed().as_secs_f64() * 1e3);
        wake = frame.wake;
        if i >= WARMUP {
            let s = view.stats();
            (created, steps, compiled) = (created + s.created, steps + s.steps, compiled + s.compiled);
            sim.push(f64::from(view.stats().sim_us) / 1e3);
        }
    }
    let steady = frames.saturating_sub(WARMUP).max(1);
    let stats = view.stats();
    let alive: u32 = r.census(&view)?.iter().map(|(_, c)| c.alive).sum();
    let engine = view.gpu_particles().map_or("none", |gpu| if gpu { "gpu" } else { "cpu" });
    let particles = json!({
        "engine": engine, "slots": stats.particles, "alive": alive, "steps": steps, "sim_ms": spread(sim),
    });
    let report = json!({
        "path": path, "adapter": r.adapter(), "tier": tier, "quality": quality, "size": [size.0, size.1],
        "render_size": [inner.0, inner.1], "frames": frames, "needed": needed, "cost_units": cost,
        "eval_us": spread(eval), "render_ms": spread(render), "passes": stats.passes, "draws": stats.draws,
        "textures": stats.textures, "cached": stats.reused, "created_after_warmup": created,
        "compiled_after_warmup": compiled,
        "particles": particles,
    });
    if json {
        println!("{report}");
        return Ok(());
    }
    let ms = |key: &str, unit: &str| ms_of(&report[key], unit);
    println!(
        "adapter      {}\ntier         {tier:?}, quality {quality:?}, {}x{} rendered at {}x{}",
        r.adapter(),
        size.0,
        size.1,
        inner.0,
        inner.1
    );
    println!("frames       {frames}, {needed} needed by the scheduler, estimated cost {cost:.2} units");
    println!("evaluation   {}\nrender       {}", ms("eval_us", "µs"), ms("render_ms", "ms"));
    println!(
        "last frame   {} passes, {} draws, {} pooled textures, {} cached parts",
        stats.passes, stats.draws, stats.textures, stats.reused
    );
    println!(
        "created      {created} GPU resources, {compiled} of them shaders, after {WARMUP} warm-up frames"
    );
    if engine != "none" {
        let per_frame = f64::from(steps) / steady as f64;
        println!(
            "particles    {engine}, {} slots, {alive} alive at the end, {per_frame:.1} steps per frame",
            stats.particles
        );
        println!(
            "simulation   {} after {WARMUP} warm-up frames",
            ms_of(&report["particles"]["sim_ms"], "ms")
        );
    }
    Ok(())
}
