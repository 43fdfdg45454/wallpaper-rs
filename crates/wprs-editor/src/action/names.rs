pub const ACTIONS: [&str; 42] = [
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
    "parts",
    "undo",
    "redo",
];

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
