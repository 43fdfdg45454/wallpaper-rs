use super::{Animate, floats, summed};
use crate::Color;
use crate::value::{Param, Vec2};
use serde_json::Value as Json;

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
        match v {
            Json::Bool(b) => Some(Param::Bool(*b)),
            Json::Number(n) => n.as_f64().map(|n| Param::Number(n as f32)),
            Json::String(s) => Some(Color::parse(s).map_or_else(|| Param::Text(s.clone()), Param::Color)),
            _ => floats(v).map(Param::Vec2).or_else(|| Color::from_json(v).map(Param::Color)),
        }
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
    fn parts(&self) -> ([f32; 4], usize) {
        match self {
            Param::Color(c) => c.parts(),
            other => other.comps(),
        }
    }
    fn with_parts(&self, parts: [f32; 4]) -> Param {
        match self {
            Param::Color(_) => Param::Color(Color(parts)),
            other => other.with_comps(parts),
        }
    }
    fn plus(&self, more: &Param) -> Param {
        match (self, more) {
            (Param::Number(n), Param::Vec2([x, y])) | (Param::Vec2([x, y]), Param::Number(n)) => {
                Param::Vec2([x + n, y + n])
            }
            (Param::Number(n), Param::Color(Color([r, g, b, a])))
            | (Param::Color(Color([r, g, b, a])), Param::Number(n)) => {
                Param::Color(Color([r + n, g + n, b + n, *a]))
            }
            (Param::Color(a), Param::Color(b)) => Param::Color(a.plus(b)),
            _ => summed(self, more),
        }
    }
}

impl Default for Param {
    fn default() -> Self {
        Param::Number(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_added_to_a_list_or_a_color_moves_every_part_but_the_alpha() {
        let color = Param::Color(Color([0.25, 0.5, 0.0, 0.5]));
        assert_eq!(color.plus(&Param::Number(0.25)), Param::Color(Color([0.5, 0.75, 0.25, 0.5])));
        assert_eq!(Param::Number(0.25).plus(&color), Param::Color(Color([0.5, 0.75, 0.25, 0.5])));
        assert_eq!(Param::Vec2([1.0, 2.0]).plus(&Param::Number(3.0)), Param::Vec2([4.0, 5.0]));
        assert_eq!(Param::Number(3.0).plus(&Param::Vec2([1.0, 2.0])), Param::Vec2([4.0, 5.0]));
        assert_eq!(Param::Number(3.0).plus(&Param::Number(0.5)), Param::Number(3.5));
        let opaque = Param::Color(Color([0.0, 0.0, 0.0, 1.0]));
        assert_eq!(color.plus(&opaque), Param::Color(Color([0.25, 0.5, 0.0, 1.0])), "the alpha went past 1");
    }
}
