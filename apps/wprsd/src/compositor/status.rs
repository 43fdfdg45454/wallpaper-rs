#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct OutputStatus {
    pub kind: &'static str,
    pub playing: bool,
    pub error: Option<String>,
    pub parallax: bool,
    pub frames: u64,
    pub page_frames: u64,
    pub fps: u32,
    pub sound: &'static str,
    pub flashes: &'static str,
    pub particles: &'static str,
    pub level: Level,
    pub decode: &'static str,
    pub left_out: Vec<String>,
    pub console: Vec<String>,
    pub sandbox: &'static str,
    pub helper: Option<u32>,
    pub hdr: super::light::Hdr,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize)]
pub struct Level {
    pub quality: &'static str,
    pub reason: &'static str,
    pub cost: f32,
    pub budget: f32,
    pub automatic: &'static str,
}

impl OutputStatus {
    pub fn shown(&self) -> wprs_core::model::Shown {
        use wprs_core::model::Shown;
        match self.kind {
            "image" => Shown::Image,
            "video" => Shown::Video,
            "scene" => Shown::Scene,
            "web" => Shown::Web,
            _ => Shown::Color,
        }
    }
}
