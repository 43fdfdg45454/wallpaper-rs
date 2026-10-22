use crate::Ease;
use crate::animate::Animate;
use serde::de::{self, Deserializer};
use serde::ser::{SerializeTuple, Serializer};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Loop {
    #[default]
    Once,
    Repeat,
    Pingpong,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timeline {
    pub id: String,
    pub duration: f32,
    #[serde(default, rename = "loop")]
    pub repeat: Loop,
    #[serde(default = "one")]
    pub speed: f32,
    #[serde(default)]
    pub delay: f32,
    #[serde(default = "yes")]
    pub autoplay: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Key<T> {
    pub t: f32,
    pub v: T,
    pub ease: Ease,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Playhead {
    pub(crate) t: f32,
    pub(crate) playing: bool,
    pub(crate) reversed: bool,
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
        (Some(a), Some(b)) => a.v.lerp(&b.v, a.ease.apply((t - a.t) / (b.t - a.t).max(1e-6))),
        (Some(k), None) | (None, Some(k)) => k.v.clone(),
        (None, None) => T::default(),
    }
}

impl Timeline {
    pub(crate) fn start(&self) -> Playhead {
        Playhead {
            t: -self.delay,
            playing: self.autoplay && self.trigger.as_deref().is_none_or(|t| t == "start"),
            reversed: false,
        }
    }

    pub(crate) fn advance(&self, head: &mut Playhead, dt: f32) {
        if head.playing {
            head.t += dt * self.speed * if head.reversed { -1.0 } else { 1.0 };
            let done =
                self.repeat == Loop::Once && (head.t >= self.duration || (head.reversed && head.t <= 0.0));
            head.playing &= !done;
        }
    }

    pub(crate) fn position(&self, head: &Playhead) -> f32 {
        local(head.t.max(0.0), self.duration, self.repeat)
    }
}

impl<T: Serialize> Serialize for Key<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let plain = self.ease == Ease::default();
        let mut tuple = s.serialize_tuple(if plain { 2 } else { 3 })?;
        tuple.serialize_element(&self.t)?;
        tuple.serialize_element(&self.v)?;
        if !plain {
            tuple.serialize_element(&self.ease)?;
        }
        tuple.end()
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Key<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr<T> {
            Plain((f32, T)),
            Eased((f32, T, Ease)),
        }
        match Repr::deserialize(d) {
            Ok(Repr::Plain((t, v))) => Ok(Key { t, v, ease: Ease::default() }),
            Ok(Repr::Eased((t, v, ease))) => Ok(Key { t, v, ease }),
            Err(_) => Err(de::Error::custom("a key is [time, value] or [time, value, easing]")),
        }
    }
}

fn one() -> f32 {
    1.0
}

fn yes() -> bool {
    true
}
