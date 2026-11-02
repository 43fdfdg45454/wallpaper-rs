use crate::Scene;

pub const DEBUG_VIEWS: [&str; 4] = ["", "mask", "matte", "shadows"];

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Drafted {
    pub debug3d: u8,
    pub split: f32,
}

impl Scene {
    pub fn drafted(&mut self) {
        let editor = self.ext.get("editor");
        let shown = editor.and_then(|e| e["debug3d"].as_str());
        let debug3d = DEBUG_VIEWS.iter().position(|k| Some(*k) == shown).unwrap_or(0) as u8;
        let split = editor.and_then(|e| e["tonemap_split"].as_f64()).unwrap_or(0.0).clamp(0.0, 1.0) as f32;
        self.draft = Drafted { debug3d, split };
    }
}
