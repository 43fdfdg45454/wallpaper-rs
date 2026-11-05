pub const EDITOR: &str = "/scene/ext/editor";

pub const ACTIONS: [&str; 54] = [
    "set",
    "add",
    "remove",
    "move",
    "add_layer",
    "remove_layer",
    "duplicate_layer",
    "move_layer",
    "group",
    "ungroup",
    "hide",
    "lock",
    "solo",
    "keyframe",
    "import",
    "import_effect",
    "import_script",
    "import_assets",
    "asset",
    "rgb",
    "translate",
    "thumbnail",
    "upgrade",
    "add_property",
    "move_property",
    "rename_property",
    "remove_property",
    "condition",
    "add_preset",
    "paint_mask",
    "mesh_from_picture",
    "bone",
    "mesh_depth",
    "weights",
    "auto_weights",
    "clip",
    "key",
    "expression",
    "ik",
    "physics",
    "wind",
    "attach",
    "point",
    "parts",
    "particles",
    "light",
    "lighting",
    "normals",
    "paint_map",
    "ambient",
    "scene3d",
    "animate",
    "undo",
    "redo",
];

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportAssets {
    pub package: std::path::PathBuf,
    #[serde(default)]
    pub parts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptFile {
    pub file: std::path::PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetEdit {
    pub asset: String,
    #[serde(default)]
    pub ktx2: bool,
    #[serde(default)]
    pub codec: Option<String>,
    #[serde(default)]
    pub quality: Option<u32>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub placeholder: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RgbEdit {
    pub layer: String,
    #[serde(default)]
    pub on: Option<bool>,
    #[serde(default)]
    pub size: Option<[f64; 2]>,
    #[serde(default)]
    pub whole: bool,
}

impl super::Action {
    pub fn name(&self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_object()?.keys().next().cloned())
            .unwrap_or_default()
    }
}

pub(super) fn half() -> f32 {
    0.5
}

pub(super) fn whole() -> f32 {
    1.0
}

pub(super) fn tenth() -> f32 {
    0.1
}
