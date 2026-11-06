use super::{Handles, Loop};
use crate::Ease;
use crate::animate::Animate;
use serde::de::{self, Deserializer, IntoDeserializer};
use serde::ser::{SerializeTuple, Serializer};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub struct Key<T> {
    pub t: f32,
    pub v: T,
    pub ease: Ease,
    pub handles: Option<Handles>,
    pub one: Option<f32>,
}

pub fn local(t: f32, d: f32, mode: Loop) -> f32 {
    if d <= 0.0 {
        return 0.0;
    }
    match mode {
        Loop::Once => t.clamp(0.0, d),
        Loop::Repeat => t.rem_euclid(d),
        Loop::Pingpong => {
            let u = t.rem_euclid(2.0 * d);
            if u > d { 2.0 * d - u } else { u }
        }
    }
}

pub fn sample<T: Animate>(keys: &[Key<T>], t: f32) -> T {
    let i = keys.partition_point(|k| k.t <= t);
    match (i.checked_sub(1).map(|i| &keys[i]), keys.get(i)) {
        (Some(a), Some(b)) => {
            curve(a, b, t).unwrap_or_else(|| a.v.lerp(&b.v, a.ease.apply((t - a.t) / (b.t - a.t).max(1e-6))))
        }
        (Some(k), None) | (None, Some(k)) => k.v.clone(),
        (None, None) => T::default(),
    }
}

fn curve<T: Animate>(a: &Key<T>, b: &Key<T>, t: f32) -> Option<T> {
    let (out, back) = (a.handles.and_then(|h| h.out), b.handles.and_then(|h| h.before));
    if out.is_none() && back.is_none() {
        return None;
    }
    let (([v0, ..], n), ([v1, ..], _)) = (a.v.comps(), b.v.comps());
    let lone = a.one.zip(b.one).filter(|_| n != 1);
    let (v0, v1) = match lone {
        Some(ends) => ends,
        None if n == 1 => (v0, v1),
        None => (0.0, 1.0),
    };
    let third = [(b.t - a.t) / 3.0, (v1 - v0) / 3.0];
    let ([dt0, dv0], [dt1, dv1]) = (out.unwrap_or(third), back.unwrap_or(third));
    let at = |p: [f32; 4], s: f32| {
        let r = 1.0 - s;
        r * r * r * p[0] + 3.0 * r * r * s * p[1] + 3.0 * r * s * s * p[2] + s * s * s * p[3]
    };
    let xs = [a.t, a.t + dt0, b.t - dt1, b.t];
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        (lo, hi) = if at(xs, mid) < t { (mid, hi) } else { (lo, mid) };
    }
    let v = at([v0, v0 + dv0, v1 - dv1, v1], (lo + hi) / 2.0);
    Some(match lone {
        Some(_) => T::from_json(&serde_json::Value::from(v)).unwrap_or_else(|| a.v.clone()),
        None if n == 1 => a.v.with_comps([v, 0.0, 0.0, 0.0]),
        None => a.v.lerp(&b.v, v),
    })
}

impl<T: Serialize> Serialize for Key<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let plain = self.ease == Ease::default() && self.handles.is_none();
        let mut tuple = s.serialize_tuple(if plain { 2 } else { 3 })?;
        tuple.serialize_element(&self.t)?;
        match self.one {
            Some(one) => tuple.serialize_element(&one)?,
            None => tuple.serialize_element(&self.v)?,
        }
        match &self.handles {
            Some(handles) => tuple.serialize_element(handles)?,
            None if !plain => tuple.serialize_element(&self.ease)?,
            None => {}
        }
        tuple.end()
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Key<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr<T> {
            Plain((f32, Read<T>)),
            Eased((f32, Read<T>, Ease)),
            Handled((f32, Read<T>, Handles)),
        }
        let key = |t, Read(v, one), ease, handles| Key { t, v, ease, handles, one };
        match Repr::deserialize(d) {
            Ok(Repr::Plain((t, v))) => Ok(key(t, v, Ease::default(), None)),
            Ok(Repr::Eased((t, v, ease))) => Ok(key(t, v, ease, None)),
            Ok(Repr::Handled((t, v, handles))) => Ok(key(t, v, Ease::default(), Some(handles))),
            Err(_) => Err(de::Error::custom(
                "a key is [time, value], [time, value, easing] or [time, value, { \"in\", \"out\" }]",
            )),
        }
    }
}

struct Read<T>(T, Option<f32>);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Read<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Either<T> {
            One(f32),
            Other(T),
        }
        match Either::deserialize(d)? {
            Either::One(x) => {
                let v = T::deserialize(IntoDeserializer::<D::Error>::into_deserializer(x))?;
                Ok(Read(v, Some(x)))
            }
            Either::Other(v) => Ok(Read(v, None)),
        }
    }
}
