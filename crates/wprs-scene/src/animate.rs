use crate::Color;
use crate::resolve::key;
use crate::value::{Param, Vec2};
use serde_json::Value as Json;

pub trait Animate: Clone + PartialEq + Default {
    fn lerp(&self, to: &Self, f: f32) -> Self {
        if f < 1.0 { self.clone() } else { to.clone() }
    }
    fn from_json(value: &Json) -> Option<Self>;
    fn from_signal(_: f32) -> Option<Self> {
        None
    }
    fn from_text(_: String) -> Option<Self> {
        None
    }
    fn comps(&self) -> ([f32; 4], usize) {
        ([0.0; 4], 0)
    }
    fn with_comps(&self, _: [f32; 4]) -> Self {
        self.clone()
    }
}

impl Animate for f32 {
    fn lerp(&self, to: &f32, f: f32) -> f32 {
        self + (to - self) * f
    }
    fn from_json(v: &Json) -> Option<f32> {
        v.as_f64().map(|v| v as f32).or_else(|| v.as_bool().map(f32::from))
    }
    fn from_signal(x: f32) -> Option<f32> {
        Some(x)
    }
    fn comps(&self) -> ([f32; 4], usize) {
        ([*self, 0.0, 0.0, 0.0], 1)
    }
    fn with_comps(&self, c: [f32; 4]) -> f32 {
        c[0]
    }
}

impl Animate for bool {
    fn from_json(v: &Json) -> Option<bool> {
        v.as_bool().or_else(|| v.as_f64().map(|n| n > 0.5))
    }
    fn from_signal(x: f32) -> Option<bool> {
        Some(x > 0.5)
    }
}

impl Animate for String {
    fn from_json(v: &Json) -> Option<String> {
        Some(key(v))
    }
    fn from_text(text: String) -> Option<String> {
        Some(text)
    }
}

impl Animate for Vec2 {
    fn lerp(&self, to: &Vec2, f: f32) -> Vec2 {
        Vec2([self.0[0].lerp(&to.0[0], f), self.0[1].lerp(&to.0[1], f)])
    }
    fn from_json(v: &Json) -> Option<Vec2> {
        serde_json::from_value(v.clone()).ok()
    }
    fn from_signal(x: f32) -> Option<Vec2> {
        Some(Vec2([x; 2]))
    }
    fn comps(&self) -> ([f32; 4], usize) {
        ([self.0[0], self.0[1], 0.0, 0.0], 2)
    }
    fn with_comps(&self, c: [f32; 4]) -> Vec2 {
        Vec2([c[0], c[1]])
    }
}

impl Animate for Color {
    fn lerp(&self, to: &Color, f: f32) -> Color {
        Color(std::array::from_fn(|i| self.0[i].lerp(&to.0[i], f)))
    }
    fn from_json(v: &Json) -> Option<Color> {
        serde_json::from_value(v.clone()).ok()
    }
    fn from_signal(x: f32) -> Option<Color> {
        Some(Color([x, x, x, 1.0]))
    }
}

impl Animate for Param {
    fn lerp(&self, to: &Param, f: f32) -> Param {
        match (self, to) {
            (Param::Number(a), Param::Number(b)) => Param::Number(a.lerp(b, f)),
            (Param::Vec2(a), Param::Vec2(b)) => Param::Vec2(Vec2(*a).lerp(&Vec2(*b), f).0),
            (Param::Color(a), Param::Color(b)) => Param::Color(a.lerp(b, f)),
            _ if f < 1.0 => self.clone(),
            _ => to.clone(),
        }
    }
    fn from_json(v: &Json) -> Option<Param> {
        serde_json::from_value(v.clone()).ok()
    }
    fn from_signal(x: f32) -> Option<Param> {
        Some(Param::Number(x))
    }
    fn comps(&self) -> ([f32; 4], usize) {
        match self {
            Param::Number(n) => n.comps(),
            Param::Vec2(v) => Vec2(*v).comps(),
            _ => ([0.0; 4], 0),
        }
    }
    fn with_comps(&self, c: [f32; 4]) -> Param {
        match self {
            Param::Number(_) => Param::Number(c[0]),
            Param::Vec2(_) => Param::Vec2([c[0], c[1]]),
            other => other.clone(),
        }
    }
}

impl Default for Param {
    fn default() -> Self {
        Param::Number(0.0)
    }
}
