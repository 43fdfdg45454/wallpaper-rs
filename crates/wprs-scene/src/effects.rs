use crate::Color;
use crate::quality::Class;
use crate::value::Param;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub const FRAME_WGSL: &str = include_str!("frame.wgsl");
pub const PRELUDE: &str = concat!(include_str!("frame.wgsl"), include_str!("prelude.wgsl"));
pub const EFFECTS_WGSL: &str = include_str!("effects.wgsl");
pub const SLOTS: usize = 16;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    pub id: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub name: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uses: Vec<Use>,
    #[serde(default)]
    pub mask_mode: MaskMode,
    #[serde(default)]
    pub params: Vec<ParamDef>,
    pub passes: Vec<PassDef>,
    #[serde(default)]
    pub cost: Cost,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuse: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reach: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Use {
    Time,
    Cursor,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaskMode {
    #[default]
    Mix,
    Strength,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamKind {
    Float,
    Vec2,
    Color,
    Bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamDef {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: ParamKind,
    pub default: Param,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PassDef {
    pub entry: String,
    #[serde(default = "full")]
    pub scale: f32,
    #[serde(default = "previous")]
    pub inputs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Cost {
    pub taps: f32,
    pub passes: u32,
    pub scale: f32,
}

impl Default for Cost {
    fn default() -> Self {
        Cost { taps: 1.0, passes: 1, scale: 1.0 }
    }
}

pub fn builtin() -> &'static [Descriptor] {
    static BUILTIN: OnceLock<Vec<Descriptor>> = OnceLock::new();
    BUILTIN.get_or_init(|| {
        serde_json::from_str(include_str!("effects.json")).expect("built-in effect descriptors")
    })
}

pub fn descriptor(id: &str) -> Option<&'static Descriptor> {
    builtin().iter().find(|d| d.id == id)
}

impl ParamDef {
    pub fn accepts(&self, value: &Param) -> bool {
        matches!(
            (self.kind, value),
            (ParamKind::Float, Param::Number(_))
                | (ParamKind::Vec2, Param::Vec2(_) | Param::Number(_))
                | (ParamKind::Color, Param::Color(_))
                | (ParamKind::Bool, Param::Bool(_))
        )
    }

    pub fn slot(&self, value: &Param) -> [f32; 4] {
        let clamp = |v: f32| v.clamp(self.min.unwrap_or(f32::MIN), self.max.unwrap_or(f32::MAX));
        match (self.kind, value) {
            (ParamKind::Float, Param::Number(v)) => [clamp(*v), 0.0, 0.0, 0.0],
            (ParamKind::Vec2, Param::Number(v)) => [clamp(*v), clamp(*v), 0.0, 0.0],
            (ParamKind::Vec2, Param::Vec2([x, y])) => [clamp(*x), clamp(*y), 0.0, 0.0],
            (ParamKind::Color, Param::Color(Color(c))) => *c,
            (ParamKind::Bool, Param::Bool(b)) => [f32::from(*b), 0.0, 0.0, 0.0],
            _ => self.slot(&self.default),
        }
    }
}

impl Descriptor {
    pub fn pack<'a>(&'a self, value: impl Fn(&'a str) -> Option<Param>) -> [[f32; 4]; SLOTS] {
        let mut slots = [[0.0; 4]; SLOTS];
        for (slot, def) in slots.iter_mut().zip(&self.params) {
            *slot = def.slot(&value(&def.id).unwrap_or_else(|| def.default.clone()));
        }
        slots
    }

    pub fn param(&self, id: &str) -> Option<usize> {
        self.params.iter().position(|p| p.id == id)
    }

    pub fn units(&self) -> f32 {
        self.cost.taps * self.cost.passes as f32 * self.cost.scale * self.cost.scale
    }

    pub fn class(&self) -> Class {
        match self.units() {
            u if u <= 1.0 => Class::Light,
            u if u <= 6.0 => Class::Medium,
            _ => Class::Heavy,
        }
    }

    pub fn animates(&self) -> bool {
        !self.uses.is_empty()
    }
}

fn full() -> f32 {
    1.0
}

fn previous() -> Vec<String> {
    vec!["previous".into()]
}
