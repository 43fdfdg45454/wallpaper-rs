use crate::RenderError;
use gpu::{Gpu, Load, Variant};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use wgpu::{Features, TextureFormat};

mod gpu;
mod set;

const REPEATS: usize = 3;

#[derive(Debug, Clone)]
pub struct Ceiling {
    pub gl: bool,
    pub size: (u32, u32),
    pub passes: usize,
    pub frames: usize,
    pub rounds: u32,
    pub only: Option<String>,
    pub repeat: usize,
    pub warm: Duration,
}

impl Default for Ceiling {
    fn default() -> Ceiling {
        let (repeat, warm) = (1, Duration::from_secs(2));
        Ceiling {
            gl: false,
            size: (1440, 3040),
            passes: 20,
            frames: 10,
            rounds: 64,
            only: None,
            repeat,
            warm,
        }
    }
}

const BASE: Variant = Variant {
    name: "base",
    format: TextureFormat::Rgba8Unorm,
    load: Load::Clear,
    store: true,
    rounds: 0,
    half: false,
    fixed: false,
    blend: false,
    one_pass: false,
    split: false,
    stamps: false,
    scale: 1,
};

fn variants(rounds: u32, half: bool) -> Vec<Variant> {
    let v = |name| Variant { name, ..BASE };
    let mut list = vec![
        v("base"),
        Variant { format: TextureFormat::Rgba16Float, ..v("rgba16f") },
        Variant { load: Load::Keep, ..v("load") },
        Variant { load: Load::DontCare, ..v("dont_care") },
        Variant { fixed: true, ..v("fixed_source") },
        Variant { fixed: true, store: false, ..v("fixed_discard") },
        Variant { fixed: true, blend: true, load: Load::Keep, ..v("blend_passes") },
        Variant { fixed: true, blend: true, one_pass: true, ..v("blend_one_pass") },
        Variant { split: true, ..v("submit_per_pass") },
        Variant { stamps: true, ..v("stamped_passes") },
        Variant { scale: 2, ..v("half_size") },
        Variant { rounds, ..v("alu_f32") },
    ];
    if half {
        list.push(Variant { rounds, half: true, ..v("alu_f16") });
        list.push(Variant { half: true, ..v("light_f16") });
    }
    list
}

pub fn ceiling(c: &Ceiling) -> Result<Value, RenderError> {
    let mut gpu = Gpu::open(if c.gl { wgpu::Backends::GL } else { wgpu::Backends::VULKAN })?;
    let (half, stamps) = (gpu.has(Features::SHADER_F16), gpu.has(Features::TIMESTAMP_QUERY));
    let (passes, frames) = (c.passes.max(1), c.frames.max(1));
    let base = gpu.set(&BASE, c.size, passes);
    let start = Instant::now();
    while start.elapsed() < c.warm {
        gpu.frame(&base, &BASE, passes);
        gpu.wait()?;
    }
    drop(base);
    let wanted = |v: &&Variant| c.only.as_deref().is_none_or(|o| o.split(',').any(|n| n == v.name));
    let (mut rows, list) = (Vec::new(), variants(c.rounds, half));
    for round in 0..c.repeat.max(1) {
        for v in list.iter().filter(wanted) {
            let mut row = measure(&mut gpu, v, c.size, passes, frames)?;
            row["round"] = json!(round);
            rows.push(row);
        }
    }
    let info = &gpu.info;
    Ok(json!({
        "adapter": format!("{} ({:?})", info.name, info.backend),
        "driver": format!("{} {}", info.driver, info.driver_info),
        "f16": half, "timestamps": stamps, "size": [c.size.0, c.size.1], "passes": passes,
        "frames": frames, "rounds": c.rounds, "variants": rows,
    }))
}

fn measure(
    gpu: &mut Gpu,
    v: &Variant,
    size: (u32, u32),
    passes: usize,
    frames: usize,
) -> Result<Value, RenderError> {
    let size = (size.0 / v.scale, size.1 / v.scale);
    let set = gpu.set(v, size, passes);
    gpu.frame(&set, v, passes);
    gpu.wait()?;
    let mut best = f64::MAX;
    for _ in 0..REPEATS {
        let at = Instant::now();
        (0..frames).for_each(|_| gpu.frame(&set, v, passes));
        gpu.wait()?;
        best = best.min(at.elapsed().as_secs_f64() * 1e3 / (frames * passes) as f64);
    }
    let stamped = gpu.stamped(&set, if v.one_pass { 1 } else { passes });
    let stamps = stamped.map(|(sum, span)| {
        json!({ "sum_ms_per_pass": sum / passes as f64, "span_ms_per_pass": span / passes as f64 })
    });
    let px = f64::from(size.0) * f64::from(size.1);
    let bpp = f64::from(v.format.block_copy_size(None).unwrap_or(4));
    let share = if v.one_pass { 1.0 / passes as f64 } else { 1.0 };
    let targets = f64::from(u8::from(v.store)) + f64::from(u8::from(v.load == Load::Keep));
    let bytes = bpp * (1.0 + share * targets);
    Ok(json!({
        "name": v.name, "size": [size.0, size.1], "ms_per_pass": best,
        "gpx_s": px / best / 1e6, "bytes_per_px": bytes, "gb_s": px * bytes / best / 1e6,
        "gfma_s": px * f64::from(v.rounds) * 4.0 / best / 1e6, "stamps": stamps,
    }))
}
