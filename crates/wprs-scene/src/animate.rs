use crate::Color;
use crate::resolve::binding::key;
use crate::value::{Vec2, Vec3};
use serde_json::Value as Json;

mod param;

pub(crate) fn floats<const N: usize>(v: &Json) -> Option<[f32; N]> {
    let list = v.as_array().filter(|list| list.len() == N)?;
    let mut out = [0.0; N];
    for (slot, x) in out.iter_mut().zip(list) {
        *slot = x.as_f64()? as f32;
    }
    Some(out)
}

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
    fn parts(&self) -> ([f32; 4], usize) {
        self.comps()
    }
    fn with_parts(&self, parts: [f32; 4]) -> Self {
        self.with_comps(parts)
    }
    fn plus(&self, more: &Self) -> Self {
        summed(self, more)
    }
}

pub(crate) fn summed<T: Animate>(base: &T, more: &T) -> T {
    let ((mut parts, n), (more, _)) = (base.parts(), more.parts());
    parts[..n].iter_mut().zip(more).for_each(|(p, m)| *p += m);
    base.with_parts(parts)
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
        v.as_f64().map(|x| Vec2([x as f32; 2])).or_else(|| floats(v).map(Vec2))
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

impl Animate for Vec3 {
    fn lerp(&self, to: &Vec3, f: f32) -> Vec3 {
        Vec3(std::array::from_fn(|i| self.0[i].lerp(&to.0[i], f)))
    }
    fn from_json(v: &Json) -> Option<Vec3> {
        v.as_f64().map(|x| Vec3([x as f32; 3])).or_else(|| floats(v).map(Vec3))
    }
    fn from_signal(x: f32) -> Option<Vec3> {
        Some(Vec3([x; 3]))
    }
}

impl Animate for Color {
    fn lerp(&self, to: &Color, f: f32) -> Color {
        crate::oklab::mix(*self, *to, f)
    }
    fn parts(&self) -> ([f32; 4], usize) {
        (self.0, 4)
    }
    fn with_parts(&self, parts: [f32; 4]) -> Color {
        Color(parts)
    }
    fn plus(&self, more: &Color) -> Color {
        let Color([r, g, b, a]) = summed(self, more);
        Color([r, g, b, a.clamp(0.0, 1.0)])
    }
    fn from_json(v: &Json) -> Option<Color> {
        match v {
            Json::String(hex) => Color::parse(hex),
            _ => floats(v).map(Color).or_else(|| floats(v).map(|[r, g, b]| Color([r, g, b, 1.0]))),
        }
    }
    fn from_signal(x: f32) -> Option<Color> {
        Some(Color([x, x, x, 1.0]))
    }
}
