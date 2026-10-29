use crate::Color;
use crate::content::{Align, white};
use crate::value::{Value, Vec2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MAX_FALLBACK: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Layout {
    pub wrap: Option<Wrap>,
    pub line_height: Option<f32>,
    pub padding: f32,
    pub backing: Option<(Color, [f32; 2], f32)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TextLayer {
    pub text: Value<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-ref" = "font"))]
    pub font: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(extend("x-ref" = "font"))]
    pub fallback: Vec<String>,
    #[serde(default = "size")]
    #[schemars(extend("x-unit" = "px", "x-range" = [1, 1024]))]
    pub size: f32,
    #[serde(default = "white")]
    pub color: Value<Color>,
    #[serde(default)]
    pub align: Align,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valign: Option<Valign>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<Wrap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [0.5, 4]))]
    pub line_height: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Backing>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px", "x-range" = [0, 1024]))]
    pub padding: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px", "x-range" = [-100, 1000]))]
    pub letter_spacing: Option<Value<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<Outline>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<Shadow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "px", "x-range" = [0, 64]))]
    pub blur: Option<Value<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render: Option<Glyphs>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Valign {
    Top,
    #[default]
    Center,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Glyphs {
    #[default]
    Auto,
    Raster,
    Sdf,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Wrap {
    #[schemars(extend("x-unit" = "px", "x-range" = [1, 16384]))]
    pub width: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-range" = [1, 1000]))]
    pub lines: Option<u32>,
    #[serde(default)]
    pub ellipsis: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Backing {
    pub color: Value<Color>,
    #[serde(default)]
    #[schemars(extend("x-unit" = "px"))]
    pub padding: [f32; 2],
    #[serde(default)]
    #[schemars(extend("x-unit" = "px"))]
    pub radius: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Outline {
    #[serde(default = "black")]
    pub color: Value<Color>,
    #[schemars(extend("x-unit" = "px", "x-range" = [0, 64]))]
    pub width: Value<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Shadow {
    #[serde(default = "black")]
    pub color: Value<Color>,
    #[serde(default = "drop")]
    #[schemars(extend("x-unit" = "px"))]
    pub offset: Value<Vec2>,
    #[serde(default = "soft")]
    #[schemars(extend("x-unit" = "px", "x-range" = [0, 64]))]
    pub blur: Value<f32>,
}

impl Valign {
    pub fn fraction(self) -> f32 {
        self as u8 as f32 / 2.0
    }
}

impl TextLayer {
    pub fn new(text: Value<String>) -> TextLayer {
        let (font, fallback, valign, wrap, line_height, background, padding) = Default::default();
        let (letter_spacing, outline, shadow, blur, render) = Default::default();
        let (size, color, align) = (size(), white(), Align::default());
        TextLayer {
            text,
            font,
            fallback,
            size,
            color,
            align,
            valign,
            wrap,
            line_height,
            background,
            padding,
            letter_spacing,
            outline,
            shadow,
            blur,
            render,
        }
    }

    pub fn laid_out(&self) -> bool {
        let boxed = self.background.is_some() || self.padding.is_some() || self.line_height.is_some();
        boxed || self.valign.is_some() || self.wrap.is_some()
    }

    pub fn styled(&self) -> bool {
        let drawn = self.outline.is_some() || self.shadow.is_some() || self.blur.is_some();
        drawn || self.letter_spacing.is_some() || self.render == Some(Glyphs::Sdf)
    }
}

fn size() -> f32 {
    48.0
}

fn black() -> Value<Color> {
    Value::Fixed(Color([0.0, 0.0, 0.0, 1.0]))
}

fn drop() -> Value<Vec2> {
    Value::Fixed(Vec2([2.0, 2.0]))
}

fn soft() -> Value<f32> {
    Value::Fixed(2.0)
}
