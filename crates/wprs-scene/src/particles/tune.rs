use super::{Instance, System, Tunable};
use crate::resolve::Context;
use crate::value::Value;
use crate::{Kind, Layer};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Tuned {
    pub alpha: f32,
    pub size: f32,
    pub count: f32,
    pub speed: f32,
    pub life: f32,
    pub time_scale: f32,
    pub brightness: f32,
    pub color: Option<[f32; 4]>,
}

fn instance(layer: Option<&Layer>) -> Option<&Instance> {
    match &layer?.kind {
        Kind::Particles { instance, .. } => instance.as_deref(),
        _ => None,
    }
}

pub(crate) fn tuned(cx: &Context, layer: Option<&Layer>, s: &System) -> Tuned {
    let own = instance(layer);
    let script = layer.and_then(|l| cx.script.particles.get(&l.id)).map(|c| c.instance).unwrap_or_default();
    let free = |t: Tunable| !s.lock.contains(&t);
    let pick = |t: Tunable, set: Option<f32>, v: Option<&Value<f32>>, max: f32| {
        let v = set.filter(|_| free(t)).or_else(|| v.map(|v| v.get(cx))).unwrap_or(1.0);
        if v.is_finite() { v.clamp(0.0, max) } else { 1.0 }
    };
    let field = |f: fn(&Instance) -> &Option<Value<f32>>| own.and_then(|i| f(i).as_ref());
    let color = script.color.filter(|_| free(Tunable::Color));
    Tuned {
        alpha: pick(Tunable::Alpha, script.alpha, field(|i| &i.alpha), 1.0),
        size: pick(Tunable::Size, script.size, field(|i| &i.size), 100.0),
        count: pick(Tunable::Count, script.count, field(|i| &i.count), 1.0),
        speed: pick(Tunable::Speed, script.speed, field(|i| &i.speed), 100.0),
        life: pick(Tunable::Life, script.life, field(|i| &i.life), 100.0),
        time_scale: pick(Tunable::TimeScale, script.time_scale, field(|i| &i.time_scale), 100.0),
        brightness: pick(Tunable::Brightness, script.brightness, field(|i| &i.brightness), 16.0),
        color: color.or_else(|| own.and_then(|i| i.color.as_ref()).map(|c| c.get(cx).0)),
    }
}

pub(crate) fn moved(cx: &Context, layer: Option<&Layer>, s: &System, k: usize) -> Option<[f32; 2]> {
    if s.lock.contains(&Tunable::Points) {
        return None;
    }
    let script = layer.and_then(|l| cx.script.particles.get(&l.id)?.points.get(&(k as u8)).copied());
    script.or_else(|| instance(layer)?.points.get(k).map(|v| v.get(cx).0))
}
