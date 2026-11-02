use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit3d {
    Model(ModelEdit),
    Group {
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        into: Option<String>,
        #[serde(default)]
        layers: Vec<String>,
    },
    Space {
        layer: String,
        deep: bool,
    },
    DepthTest {
        layer: String,
        on: bool,
    },
    Material(MaterialEdit),
    Camera(CameraEdit),
    View(ViewEdit),
    Tones(TonesEdit),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ModelEdit {
    Add {
        asset: String,
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        into: Option<String>,
        #[serde(default)]
        at: Option<[f64; 3]>,
    },
    Set(ModelSet),
    Remove {
        layer: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModelSet {
    pub layer: String,
    pub position: Option<[f64; 3]>,
    pub rotation: Option<[f64; 3]>,
    pub scale: Option<[f64; 3]>,
    pub size: Option<[f64; 2]>,
    pub env: Option<String>,
    pub asset: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MaterialEdit {
    pub layer: String,
    pub name: String,
    pub shader: Option<String>,
    pub lit: Option<bool>,
    pub double_sided: Option<bool>,
    pub cast_shadows: Option<bool>,
    pub receive_shadows: Option<bool>,
    pub color: Option<Value>,
    pub emissive: Option<Value>,
    pub params: BTreeMap<String, f64>,
    pub clear: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CameraEdit {
    pub fov: Option<f64>,
    pub near: Option<f64>,
    pub far: Option<f64>,
    pub eye: Option<[f64; 3]>,
    pub center: Option<[f64; 3]>,
    pub up: Option<[f64; 3]>,
    pub roll: Option<f64>,
    pub from_view: bool,
    pub clear: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ViewEdit {
    pub orbit: Option<[f64; 2]>,
    pub pan: Option<[f64; 2]>,
    pub zoom: Option<f64>,
    pub through: Option<bool>,
    pub frame: Option<String>,
    pub reset: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TonesEdit {
    pub hdr: Option<bool>,
    pub tonemap: Option<String>,
    pub split: Option<f64>,
}
