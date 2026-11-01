use clap::Args;
use serde_json::{Value, json};
use wprs_editor::actions::ParticleEdit as P;

#[derive(Args, Debug)]
pub struct Turbulent {
    #[arg(long, allow_hyphen_values = true, help = "Degrees from -360 to 360")]
    pub forward: Option<f64>,
    #[arg(long, help = "Half turns the noise may turn the direction, 0 to 1")]
    pub scale: Option<f64>,
    #[arg(long, help = "How fast the noise moves, 0 to 100")]
    pub noise_speed: Option<f64>,
    #[arg(long, allow_hyphen_values = true, help = "How far apart particles born together read the noise")]
    pub offset: Option<f64>,
    #[arg(long, allow_hyphen_values = true, value_name = "MIN,MAX", help = "Pixels a second")]
    pub speed: Option<String>,
    #[arg(long, allow_hyphen_values = true)]
    pub phase: Option<f64>,
    #[arg(long, help = "A band such as bass, mid, treble, level, beat or 0 to 63 that multiplies the phase")]
    pub audio: Option<String>,
    #[arg(long, help = "Stop following the sound", conflicts_with = "audio")]
    pub quiet: bool,
    #[arg(long, help = "Remove the turbulent start")]
    pub off: bool,
}

#[derive(Args, Debug)]
pub struct Offset {
    #[arg(long, help = "Pixels")]
    pub distance: Option<f64>,
    #[arg(long, help = "Noise cells per 100 pixels, 0 to 100")]
    pub scale: Option<f64>,
    #[arg(long, allow_hyphen_values = true, value_name = "X,Y", help = "-1, 0 or 1 on each axis")]
    pub sign: Option<String>,
    #[arg(long, help = "1 to 6")]
    pub octaves: Option<u32>,
    #[arg(long, help = "How far apart particles read the noise, 0 to 100")]
    pub noise_scale: Option<f64>,
    #[arg(long, help = "How fast the pattern drifts, 0 to 100")]
    pub noise_speed: Option<f64>,
    #[arg(long, help = "Remove the fractal offset")]
    pub off: bool,
}

#[derive(Args, Debug)]
pub struct Pace {
    #[arg(long, help = "Only this emitter instead of all of them")]
    pub emitter: Option<usize>,
    #[arg(long, help = "Particles an emitter starts at most each step")]
    pub max_per_step: Option<u32>,
}

fn pair<T: std::str::FromStr>(p: &str) -> Option<[T; 2]> {
    p.split_once(',').and_then(|(x, y)| Some([x.trim().parse().ok()?, y.trim().parse().ok()?]))
}

fn object(pairs: &[(&str, Option<Value>)]) -> Option<Value> {
    Some(Value::Object(pairs.iter().filter_map(|(k, v)| Some((k.to_string(), v.clone()?))).collect()))
}

fn band(a: &str) -> Value {
    a.parse::<u32>().map_or_else(|_| json!(a), |n| json!(n))
}

pub(crate) fn turbulent(system: String, t: Turbulent) -> P {
    let turbulent = object(&[
        ("forward", t.forward.map(|v| json!(v))),
        ("scale", t.scale.map(|v| json!(v))),
        ("noise_speed", t.noise_speed.map(|v| json!(v))),
        ("offset", t.offset.map(|v| json!(v))),
        ("speed", t.speed.as_deref().map(|s| pair::<f64>(s).map_or(json!(s), |p| json!(p)))),
        ("phase", t.phase.map(|v| json!(v))),
        ("audio", t.audio.as_deref().map(band)),
    ]);
    P::Turbulent { system, turbulent, quiet: t.quiet, off: t.off }
}

pub(crate) fn offset(system: String, o: Offset) -> P {
    let offset = object(&[
        ("distance", o.distance.map(|v| json!(v))),
        ("scale", o.scale.map(|v| json!(v))),
        ("sign", o.sign.as_deref().map(|s| pair::<i64>(s).map_or(json!(s), |p| json!(p)))),
        ("octaves", o.octaves.map(|v| json!(v))),
        ("noise_scale", o.noise_scale.map(|v| json!(v))),
        ("noise_speed", o.noise_speed.map(|v| json!(v))),
    ]);
    P::Offset { system, offset, off: o.off }
}

pub(crate) fn pace(system: String, p: Pace) -> P {
    P::Pace { system, emitter: p.emitter, max_per_step: p.max_per_step }
}
