use crate::editing::dimensions;
use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use wprs_render::{Bench, Level, Renderer, WARMUP};
use wprs_scene::{PHONE_SHARE, Runtime};

use crate::benching::{load, ms_of, passes};

#[derive(Subcommand, Debug)]
pub enum SceneCommand {
    #[command(about = "Render a scene package offscreen and report the cost of each frame; needs no daemon")]
    Bench {
        #[arg(help = "A scene package (.wpak)")]
        path: PathBuf,
        #[arg(long, default_value_t = 300, help = "Number of frames to render")]
        frames: usize,
        #[arg(long, default_value = "1920x1080", value_name = "WxH", help = "Output size")]
        size: String,
        #[arg(long, default_value_t = 60, help = "Frames per second of the simulated clock")]
        fps: u32,
        #[arg(long, value_parser = ["q0", "q1", "q2", "q3", "auto"], help = "Quality level; picked like the daemon when omitted, steered by GPU time like the phone with auto")]
        quality: Option<String>,
        #[arg(long, default_value_t = 1, help = "Number of screens that show the scene at once")]
        screens: usize,
        #[arg(long, help = "Print the report as JSON")]
        json: bool,
        #[arg(long, value_parser = ["t0", "t1", "t2", "t3"], help = "Apply the limits of this device tier")]
        tier: Option<String>,
        #[arg(long, help = "Time each render pass with GPU timestamps, where the GPU has them")]
        passes: bool,
        #[arg(long, value_name = "FILE.png", help = "Save the last frame as a PNG image")]
        shot: Option<PathBuf>,
        #[arg(
            long,
            value_name = "SECONDS",
            help = "Render in real time at --fps for this long instead of frame after frame"
        )]
        paced: Option<f64>,
    },
    #[command(flatten)]
    Author(Box<crate::author::Author>),
}

pub fn run(command: SceneCommand) -> Result<()> {
    let (path, bench, json, forced) = match command {
        SceneCommand::Bench {
            path,
            frames,
            size,
            fps,
            quality,
            screens,
            json,
            tier,
            passes,
            shot,
            paced,
        } => {
            let level = match quality.as_deref() {
                None => Level::Fitted,
                Some("auto") => Level::Timed(PHONE_SHARE),
                Some(q) => Level::Fixed(serde_json::from_value(json!(q))?),
            };
            let paced = paced.map(|s| Duration::from_secs_f64(s.clamp(0.1, 3600.0)));
            let (size, screens) = (dimensions(&size)?, screens.clamp(1, 16));
            let bench = Bench {
                frames,
                size,
                fps,
                level,
                tier: None,
                screens,
                passes,
                shot,
                paced,
                ..Bench::default()
            };
            (path, bench, json, tier)
        }
        SceneCommand::Author(command) => return crate::authoring::run(*command),
    };
    let (scene, values, assets) = load(&path)?;
    let gpu =
        wprs_core::Config::default_path().and_then(|p| wprs_core::Config::load(&p)).map(|c| c.render.gpu);
    let r = Renderer::with_choice(None, &gpu.unwrap_or_default())?.renderer;
    let unit_ms = r.calibrate(wprs_core::gpu_tier().as_deref()).unit_ms;
    if let Some(t) = forced {
        r.force_tier(Some(serde_json::from_value(json!(t))?));
    }
    let runtime = Runtime::new(Arc::new(scene), values);
    let mut report = wprs_render::scene_bench(&r, runtime, assets, &bench)?;
    (report["path"], report["unit_ms"]) = (json!(path), json!(unit_ms));
    if json {
        println!("{report}");
        return Ok(());
    }
    print!("{}", text(&report));
    Ok(())
}

fn text(report: &Value) -> String {
    let ms = |key: &str, unit: &str| ms_of(&report[key], unit);
    let (n, f) =
        (|key: &str| report[key].as_u64().unwrap_or(0), |key: &str| report[key].as_f64().unwrap_or(0.0));
    let size = |key: &str| format!("{}x{}", report[key][0], report[key][1]);
    let unit_ms = f("unit_ms");
    let unit = if unit_ms > 0.0 { format!("{unit_ms:.3} ms a unit") } else { "from its model".into() };
    let mut out = format!("adapter      {}\n", report["adapter"].as_str().unwrap_or_default());
    out += &format!(
        "tier         {} ({unit}), quality {}, {} rendered at {}\n",
        report["tier"],
        report["quality"],
        size("size"),
        size("render_size")
    );
    let needed =
        report["needed"].as_u64().map_or(String::new(), |k| format!(", {k} needed by the scheduler"));
    out += &format!("frames       {}{needed}, estimated cost {:.2} units\n", n("frames"), f("cost_units"));
    out += &format!("evaluation   {}\n", ms("eval_us", "µs"));
    if report["render_ms"].is_object() {
        out += &format!("render       {}\n", ms("render_ms", "ms"));
    }
    out += &format!("gpu          {}\n", ms("gpu_ms", "ms"));
    if let Some(paced) = report["paced"].as_object() {
        let at = |key: &str| paced.get(key).and_then(Value::as_f64).unwrap_or(0.0);
        out += &format!(
            "paced        {:.1} frames a second of {} over {:.1} s, {} slots missed\n",
            at("fps"),
            at("target_fps"),
            at("seconds"),
            at("missed_slots")
        );
        out += &format!("cpu          {}\n", ms("cpu_ms", "ms"));
    }
    if let Some(levels) = report["auto"]["levels"].as_array() {
        let steps: Vec<String> =
            levels.iter().map(|l| format!("{} at {:.1} s", l[1], l[0].as_f64().unwrap_or(0.0))).collect();
        out += &format!("auto         {}\n", steps.join(", "));
    }
    let first =
        report["first_ms"].as_f64().map_or(String::new(), |ms| format!(", {ms:.1} ms for the first frame"));
    out += &format!("start        {:.1} ms to load{first} of {} screens\n", f("load_ms"), n("screens"));
    out += &format!("memory       {:.1} MiB on the GPU, {:.1} MiB resident\n", f("gpu_mib"), f("rss_mib"));
    out += &format!(
        "last frame   {} passes, {} draws, {} pooled textures, {} cached parts\n",
        n("passes"),
        n("draws"),
        n("textures"),
        n("cached")
    );
    out += &format!(
        "created      {} GPU resources, {} of them shaders, after {WARMUP} warm-up frames\n",
        n("created_after_warmup"),
        n("compiled_after_warmup")
    );
    for note in report["left_out"].as_array().into_iter().flatten() {
        out += &format!("left out     {}\n", note.as_str().unwrap_or_default());
    }
    out += &passes(report);
    let particles = &report["particles"];
    let engine = particles["engine"].as_str().unwrap_or("none");
    if engine != "none" {
        let steady = n("frames").saturating_sub(WARMUP as u64).max(1);
        let per_frame = particles["steps"].as_f64().unwrap_or(0.0) / steady as f64;
        out += &format!(
            "particles    {engine}, {} slots, {} alive at the end, {per_frame:.1} steps per frame\n",
            particles["slots"], particles["alive"]
        );
        out += &format!(
            "simulation   {}, {} of it on the GPU, after {WARMUP} warm-up frames\n",
            ms_of(&particles["sim_ms"], "ms"),
            ms_of(&particles["sim_gpu_ms"], "ms")
        );
    }
    out
}
