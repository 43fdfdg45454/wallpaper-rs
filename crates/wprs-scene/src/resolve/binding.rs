use crate::animate::Animate;
use crate::resolve::Context;
use crate::value::{Binding, Value};
use crate::{Values, Wake};
use serde_json::Value as Json;

impl<T: Animate> Value<T> {
    pub fn settled(&self, props: Option<&Values>) -> Option<T> {
        let b = match self {
            Value::Fixed(v) => return Some(v.clone()),
            Value::Bound(b) => b,
        };
        let id = b.prop.as_ref().filter(|_| b.keys.is_none())?;
        let value = shaped(b, prop(b, id, props?));
        Some(b.base.as_ref().map_or_else(|| value.clone(), |base| added(base, &value)))
    }

    pub(crate) fn get(&self, cx: &Context) -> T {
        match self {
            Value::Fixed(v) => v.clone(),
            Value::Bound(b) => b.eval(cx),
        }
    }
}

impl<T: Animate> Binding<T> {
    pub(crate) fn eval(&self, cx: &Context) -> T {
        let b = self;
        let raw = if b.tracked() {
            let base = b.base.clone().unwrap_or_default();
            let (mut parts, _) = base.parts();
            for (name, track) in b.tracks() {
                let i = ["x", "y", "r", "g", "b", "a"].iter().position(|n| *n == name).unwrap_or(0);
                track.iter().for_each(|v| parts[if i < 2 { i } else { i - 2 }] = v.get(cx));
            }
            base.with_parts(parts)
        } else if let Some(keys) = &b.keys {
            crate::cue::wrapped(keys, cx.keys_time(b, keys.last().map_or(0.0, |k| k.t)), cx.wrap(b))
        } else if let Some(id) = &b.prop {
            prop(b, id, cx.props)
        } else if let Some(pattern) = &b.clock {
            match T::from_signal(0.0).is_some() && crate::CLOCKS.contains(&pattern.as_str()) {
                true => T::from_signal(cx.time_of_day(pattern)).unwrap_or_default(),
                false => T::from_text(cx.clock(pattern, false)).unwrap_or_default(),
            }
        } else if let Some(pattern) = &b.date {
            T::from_text(cx.clock(pattern, true)).unwrap_or_default()
        } else if let Some(path) = &b.script {
            let key = b.per_binding().then(|| crate::script_key(path, b.call.as_deref(), b.params.as_ref()));
            cx.script.values.get(key.as_deref().unwrap_or(path)).and_then(T::from_json).unwrap_or_default()
        } else {
            cx.signal(b).and_then(T::from_signal).unwrap_or_default()
        };
        let value = match b.base.as_ref().filter(|_| !b.tracked()) {
            Some(base) => added(base, &shaped(b, raw)),
            None => shaped(b, raw),
        };
        let times = b.smooth.map(crate::Smooth::times).filter(|t| t.iter().any(|v| *v > 0.0));
        let Some([up, down]) = times else { return value };
        let (comps, n) = value.comps();
        let mut memo = cx.memo.borrow_mut();
        let last = memo.entry(std::ptr::from_ref(b).addr()).or_insert(comps);
        last[..n].iter_mut().zip(&comps).for_each(|(l, c)| *l = crate::ease(*l, *c, cx.dt, up, down));
        if last[..n].iter().zip(&comps).any(|(l, c)| (l - c).abs() > 1e-3) {
            cx.note(Wake::Frame);
        }
        value.with_comps(*last)
    }
}

fn prop<T: Animate>(b: &Binding<T>, id: &str, props: &Values) -> T {
    let value = props.get(id).unwrap_or(&Json::Null);
    let mapped = b.map.as_ref().and_then(|map| match value {
        Json::String(text) => map.get(text.as_str()),
        Json::Bool(on) => map.get(if *on { "true" } else { "false" }),
        Json::Null => map.get("null"),
        other => map.get(&key(other)),
    });
    let mapped = mapped.cloned();
    mapped.or_else(|| T::from_json(value)).unwrap_or_default()
}

fn shaped<T: Animate>(b: &Binding<T>, raw: T) -> T {
    let (mut comps, n) = raw.comps();
    comps[..n].iter_mut().for_each(|c| *c = b.modify(*c));
    if n == 0 { raw } else { raw.with_comps(comps) }
}

fn added<T: Animate>(base: &T, value: &T) -> T {
    let ((mut parts, n), (more, _)) = (base.parts(), value.parts());
    parts[..n].iter_mut().zip(more).for_each(|(p, m)| *p += m);
    base.with_parts(parts)
}

pub(crate) fn key(value: &Json) -> String {
    match value {
        Json::String(s) => s.clone(),
        other => other.to_string(),
    }
}
