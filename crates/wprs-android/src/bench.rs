use crate::{pack, power, props, store};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use wprs_render::{Bench, Level, Renderer};
use wprs_scene::{Leveling, PHONE_SHARE, Quality, Runtime};

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
            "--size" => {
                let (w, h) = value()?.split_once('x').ok_or("--size takes WxH")?;
                bench.size = (number(w)?, number(h)?);
            }
            "--level" => {
                bench.level = match value()?.as_str() {
                    "auto" => Level::Timed(PHONE_SHARE),
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
