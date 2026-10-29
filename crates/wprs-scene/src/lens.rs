use crate::Ease;
use crate::value::{Value, Vec3};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MAX_PATHS: usize = 16;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Camera3d {
    #[schemars(extend("x-unit" = "°", "x-range" = [1, 179]))]
    pub fov: Value<f32>,
    #[schemars(extend("x-range" = [0.001, 1000]))]
    pub near: f32,
    #[schemars(extend("x-range" = [0.01, 1000000]))]
    pub far: f32,
    pub eye: Value<Vec3>,
    pub center: Value<Vec3>,
    pub up: Vec3,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<CameraPath>,
    pub order: PathOrder,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fade: Option<Fade>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PathOrder {
    #[default]
    Sequential,
    Random,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CameraPath {
    pub id: String,
    pub keys: Vec<CameraKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ease: Option<Ease>,
    #[serde(default, rename = "loop", skip_serializing_if = "std::ops::Not::not")]
    pub closed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CameraKey {
    #[schemars(extend("x-unit" = "s"))]
    pub t: f32,
    pub eye: Vec3,
    pub center: Vec3,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub up: Option<Vec3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-unit" = "°", "x-range" = [1, 179]))]
    pub fov: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FadeTo {
    #[default]
    Black,
    Cross,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Fade {
    #[serde(default)]
    pub to: FadeTo,
    #[schemars(extend("x-unit" = "s", "x-range" = [0, 10]))]
    pub duration: f32,
}

impl Default for Camera3d {
    fn default() -> Self {
        Camera3d {
            fov: Value::Fixed(60.0),
            near: 0.1,
            far: 1000.0,
            eye: Value::Fixed(Vec3([0.0, 0.0, 5.0])),
            center: Value::Fixed(Vec3([0.0; 3])),
            up: Vec3([0.0, 1.0, 0.0]),
            paths: Vec::new(),
            order: PathOrder::Sequential,
            fade: None,
        }
    }
}
