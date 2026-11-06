use crate::{pack, power, props, store};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use wprs_render::{Bench, Ceiling, Level, Renderer, Stamping};
use wprs_scene::{Leveling, Quality, Runtime};

pub(crate) fn run(path: &Path, args: &[String]) -> Result<Value, String> {
    let bench = parse(args)?;
    let pack = pack::load(path).map_err(|e| e.to_string())?;
    let mut runtime = Runtime::new(Arc::clone(&pack.scene), props::values(&pack));
    pack.inputs.into_iter().for_each(|allowed| runtime.declare(allowed));
    runtime.set_leveling(Leveling::of(&pack.properties));
    runtime.set_language(pack.locale.as_deref().unwrap_or_default());
    runtime.set_battery(true);
    let films = pack.videos.iter().filter_map(|(id, path)| Some((id.clone(), wprs_scene::film_file(path)?)));
    runtime.set_lengths(films.collect());
    let r = Renderer::with_choice(None, &Default::default()).map_err(|e| e.to_string())?.renderer;
    let unit_ms = r.calibrate(store::dir().ok().map(|dir| dir.join("gpu-tier.json")).as_deref()).unit_ms;
    let tier = power::tier(r.software(), r.compute()).min(r.tier());
    let bench = Bench { tier: Some(tier), ..bench };
    let mut report =
        wprs_render::scene_bench(&r, runtime, pack.assets.clone(), &bench).map_err(|e| e.to_string())?;
    (report["path"], report["unit_ms"]) = (json!(path), json!(unit_ms));
    Ok(report)
}

pub(crate) fn ceiling(args: &[String]) -> Result<Value, String> {
    let mut c = Ceiling::default();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--passes" => c.passes = number(value()?)?,
            "--frames" => c.frames = number(value()?)?,
            "--rounds" => c.rounds = number(value()?)?,
            "--only" => c.only = Some(value()?.clone()),
            "--repeat" => c.repeat = number(value()?)?,
            "--warm" => {
                c.warm = std::time::Duration::from_secs_f64(number::<f64>(value()?)?.clamp(0.0, 60.0))
            }
            "--size" => c.size = size(value()?)?,
            "--backend" => {
                c.gl = match value()?.as_str() {
                    "vulkan" => false,
                    "gl" => true,
                    other => return Err(format!("--backend takes vulkan or gl, not {other}")),
                }
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    wprs_render::ceiling(&c).map_err(|e| e.to_string())
}

fn size(text: &str) -> Result<(u32, u32), String> {
    let (w, h) = text.split_once('x').ok_or("--size takes WxH")?;
    Ok((number(w)?, number(h)?))
}

fn parse(args: &[String]) -> Result<Bench, String> {
    let mut bench = Bench { size: (1440, 3040), frames: 240, ..Bench::default() };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--frames" => bench.frames = number(value()?)?,
            "--fps" => bench.fps = number(value()?)?,
            "--screens" => bench.screens = number::<usize>(value()?)?.clamp(1, 16),
            "--paced" => bench.paced = Some(std::time::Duration::from_secs_f64(number(value()?)?)),
            "--shot" => bench.shot = Some(value()?.into()),
            "--passes" => bench.passes = true,
            "--overlap" => bench.overlap = true,
            "--stamps" => {
                bench.stamps = match value()?.as_str() {
                    "all" => Stamping::All,
                    "frame" => Stamping::Frame,
                    "off" => Stamping::Off,
                    other => return Err(format!("--stamps takes all, frame or off, not {other}")),
                }
            }
            "--size" => bench.size = size(value()?)?,
            "--level" => {
                bench.level = match value()?.as_str() {
                    "auto" => Level::Auto,
                    "fitted" => Level::Fitted,
                    q => {
                        Level::Fixed(serde_json::from_value::<Quality>(json!(q)).map_err(|e| e.to_string())?)
                    }
                }
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(bench)
}

fn number<T: std::str::FromStr>(text: &str) -> Result<T, String> {
    text.parse().map_err(|_| format!("{text} is not a number"))
}
