use crate::forcing::{Child, Force, Kin};
use crate::sowing::Sowing;
use serde_json::{Map, Value, json};
use wprs_editor::actions::{ParticleEdit as P, SowEdit as S, TendEdit};

pub(crate) fn pair(text: &str) -> Result<[f64; 2], String> {
    let parsed =
        text.split_once(',').and_then(|(a, b)| Some([a.trim().parse().ok()?, b.trim().parse().ok()?]));
    parsed.ok_or_else(|| format!("{text} is not two numbers A,B"))
}

pub(crate) fn scale_key(text: &str) -> Result<String, String> {
    let fits = text
        .split_once(':')
        .is_some_and(|(t, v)| t.trim().parse::<f64>().is_ok() && v.trim().parse::<f64>().is_ok());
    if fits || text == "none" { Ok(text.to_owned()) } else { Err(format!("{text} is not TIME:SCALE")) }
}

pub(crate) fn color_stop(text: &str) -> Result<String, String> {
    let fits =
        text.split_once(':').is_some_and(|(t, v)| t.trim().parse::<f64>().is_ok() && !v.trim().is_empty());
    if fits || text == "none" { Ok(text.to_owned()) } else { Err(format!("{text} is not TIME:COLOR")) }
}

pub(crate) fn sweep(text: &str) -> Result<[[f64; 2]; 2], String> {
    let (a, b) = text.split_once(':').ok_or_else(|| format!("{text} is not X,Y:X,Y"))?;
    Ok([pair(a)?, pair(b)?])
}

fn parsed<T: serde::de::DeserializeOwned>(text: &str) -> Option<T> {
    serde_json::from_value(json!(text)).ok()
}

fn keyed(list: &[String], value: fn(&str) -> Option<Value>) -> Vec<(f64, Value)> {
    let one =
        |k: &String| k.split_once(':').and_then(|(t, v)| Some((t.trim().parse().ok()?, value(v.trim())?)));
    list.iter().filter(|k| *k != "none").filter_map(one).collect()
}

fn target(text: &str) -> Value {
    pair(text).map_or_else(|_| json!(text), |p| json!(p))
}

fn force(what: Force) -> (Option<usize>, Option<Value>) {
    match what {
        Force::Gravity { pull, index } => (index, Some(json!({ "gravity": pull }))),
        Force::Drag { amount, index } => (index, Some(json!({ "drag": amount }))),
        Force::Turbulence { strength, scale, speed, index } => {
            let mut t = json!({ "strength": strength, "scale": scale });
            speed.iter().for_each(|s| t["speed"] = json!(s));
            (index, Some(json!({ "turbulence": t })))
        }
        Force::Oscillate { what, freq, scale, phase, index } => {
            let mut o = json!({ "what": what, "freq": freq, "scale": scale });
            phase.iter().for_each(|p| o["phase"] = json!(p));
            (index, Some(json!({ "oscillate": o })))
        }
        Force::Attractor { target: t, strength, radius, falloff, index } => {
            let mut a = json!({ "target": target(&t), "strength": strength });
            radius.iter().for_each(|r| a["radius"] = json!(r));
            falloff.iter().for_each(|f| a["falloff"] = json!(f));
            (index, Some(json!({ "attractor": a })))
        }
        Force::Remove { index } => (Some(index), None),
    }
}

fn kin(k: Kin) -> Value {
    let mut c = json!({ "system": k.system, "on": k.on });
    k.probability.iter().for_each(|p| c["probability"] = json!(p));
    k.max.iter().for_each(|m| c["max"] = json!(m));
    k.offset.iter().for_each(|o| c["offset"] = json!(o));
    k.angle.iter().for_each(|a| c["angle"] = json!(a));
    k.scale.iter().for_each(|s| c["scale"] = json!(s));
    if k.points_from {
        c["points_from"] = json!(true);
    }
    c
}

pub(crate) fn edit(system: String, what: Sowing) -> P {
    let layered = |l: Option<String>| l.filter(|l| !l.is_empty());
    P::Sow(match what {
        Sowing::Create { from, parent, on } => {
            S::Create { system, from, parent, on: on.as_deref().and_then(parsed) }
        }
        Sowing::Delete => S::Delete { system },
        Sowing::Settings { capacity, seed, prewarm, space, rate, emitter } => S::Settings {
            system,
            capacity,
            seed,
            prewarm,
            space: space.as_deref().and_then(parsed),
            rate,
            emitter,
        },
        Sowing::Start {
            life,
            speed,
            angle,
            size,
            rotation,
            spin,
            frame,
            color,
            alpha,
            velocity,
            radial,
            clear,
        } => {
            let mut ranges = Map::new();
            let numbers =
                [("life", life), ("speed", speed), ("angle", angle), ("size", size), ("rotation", rotation)];
            let more = [("spin", spin), ("frame", frame), ("alpha", alpha), ("radial", radial)];
            numbers
                .into_iter()
                .chain(more)
                .for_each(|(k, v)| v.iter().for_each(|v| drop(ranges.insert(k.into(), json!(v)))));
            let colors = color.as_deref().map(|c| c.split(',').map(str::trim).collect::<Vec<_>>());
            colors.iter().for_each(|c| drop(ranges.insert("color".into(), json!(c))));
            velocity.iter().for_each(|v| drop(ranges.insert("velocity".into(), json!(v))));
            S::Start { system, ranges, clear }
        }
        Sowing::Force(f) => {
            let (index, force) = force(f);
            let remove = force.is_none();
            return P::Tend(TendEdit::Force { system, index, force, remove });
        }
        Sowing::Child(Child::Add(k)) => S::Child { system, index: None, child: Some(kin(k)), remove: false },
        Sowing::Child(Child::Set { index, kin: k }) => {
            S::Child { system, index: Some(index), child: Some(kin(k)), remove: false }
        }
        Sowing::Child(Child::Remove { index }) => {
            S::Child { system, index: Some(index), child: None, remove: true }
        }
        Sowing::SizeOverLife { keys, layer } => {
            let size = keyed(&keys, |v| v.parse::<f64>().ok().map(|n| json!(n)));
            let size = size.into_iter().filter_map(|(t, v)| Some([t, v.as_f64()?])).collect();
            S::Over { system, layer: layered(layer), size: Some(size), color: None }
        }
        Sowing::ColorOverLife { stops, layer } => {
            let color = keyed(&stops, |v| Some(json!(v)));
            let color = color.into_iter().filter_map(|(t, v)| Some((t, v.as_str()?.to_owned()))).collect();
            S::Over { system, layer: layered(layer), size: None, color: Some(color) }
        }
        Sowing::Render { stretch, frames, fps, mix, brightness, layer } => {
            let stretch = stretch.map(|s| {
                let n: Vec<f64> = s.split(',').filter_map(|v| v.trim().parse().ok()).collect();
                match n.as_slice() {
                    [t] => json!(t),
                    [t, lo, hi] => json!({ "time": t, "min": lo, "max": hi }),
                    _ => json!(s),
                }
            });
            S::Render { system, layer: layered(layer), stretch, frames, fps, mix, brightness }
        }
    })
}
