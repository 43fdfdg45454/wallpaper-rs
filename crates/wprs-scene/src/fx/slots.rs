use crate::effects::{ParamDef, ParamKind};
use crate::value::Param;
use crate::{Blend, Color};

impl ParamKind {
    pub fn asset(self) -> bool {
        matches!(self, ParamKind::Texture | ParamKind::Lut | ParamKind::Mask)
    }
}

impl ParamDef {
    pub fn accepts(&self, value: &Param) -> bool {
        match (self.kind, value) {
            (ParamKind::Int, Param::Number(n)) => n.fract() == 0.0,
            (ParamKind::Enum, Param::Number(n)) => self.values.iter().any(|e| e.value == *n),
            (ParamKind::Blend, Param::Text(name)) => Blend::named(name).is_some(),
            (kind, value) => matches!(
                (kind, value),
                (ParamKind::Float, Param::Number(_))
                    | (ParamKind::Vec2 | ParamKind::Position, Param::Vec2(_) | Param::Number(_))
                    | (ParamKind::Color, Param::Color(_))
                    | (ParamKind::Bool, Param::Bool(_))
                    | (ParamKind::Texture | ParamKind::Lut | ParamKind::Mask, Param::Text(_))
            ),
        }
    }

    pub fn named<'a>(&'a self, e: &'a crate::EffectUse) -> Option<&'a str> {
        match (e.params.get(&self.id), &self.default) {
            (Some(crate::Value::Fixed(Param::Text(id))), _) | (None, Param::Text(id)) => Some(id),
            _ => None,
        }
    }

    pub fn slot(&self, value: &Param) -> [f32; 4] {
        let clamp = |v: f32| v.clamp(self.min.unwrap_or(f32::MIN), self.max.unwrap_or(f32::MAX));
        match (self.kind, value) {
            (ParamKind::Float | ParamKind::Int | ParamKind::Enum, Param::Number(v)) => {
                [clamp(*v), 0.0, 0.0, 0.0]
            }
            (ParamKind::Vec2 | ParamKind::Position, Param::Number(v)) => [clamp(*v), clamp(*v), 0.0, 0.0],
            (ParamKind::Vec2 | ParamKind::Position, Param::Vec2([x, y])) => [clamp(*x), clamp(*y), 0.0, 0.0],
            (ParamKind::Blend, Param::Text(name)) => {
                [Blend::named(name).map_or(0.0, |b| b as u8 as f32), 0.0, 0.0, 0.0]
            }
            (ParamKind::Color, Param::Color(Color([r, g, b, a]))) => {
                let [r, g, b] = [r, g, b]
                    .map(|c| if *c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) });
                [r, g, b, *a]
            }
            (ParamKind::Bool, Param::Bool(b)) => [f32::from(*b), 0.0, 0.0, 0.0],
            (ParamKind::Texture | ParamKind::Lut | ParamKind::Mask, _) => [0.0; 4],
            _ => self.slot(&self.default),
        }
    }
}

pub(crate) fn live(slot: [f32; 4]) -> bool {
    slot.iter().any(|v| v.abs() > 1e-3)
}
