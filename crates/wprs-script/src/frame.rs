use serde_json::json;
use wprs_scene::{AudioFrame, PointerState, Values};

pub struct Frame<'a> {
    pub time: f64,
    pub dt: f32,
    pub props: &'a Values,
    pub audio: &'a AudioFrame,
    pub pointer: &'a PointerState,
    pub actions: &'a [String],
}

impl Frame<'_> {
    pub fn json(&self) -> String {
        let (a, p) = (self.audio, self.pointer);
        let [x, y] = p.pos.unwrap_or(p.smooth);
        let (left, right, average) = (&a.left[..], &a.right[..], &a.average[..]);
        let audio = json!({"left": left, "right": right, "average": average, "bass": a.bass, "mid": a.mid,
            "treble": a.treble, "level": a.level, "beat": a.beat});
        let cursor = json!({"x": x, "y": y, "inside": p.inside, "buttons": p.buttons});
        let actions = self.actions;
        json!({"time": self.time, "dt": self.dt, "props": self.props, "audio": audio, "cursor": cursor, "actions": actions})
            .to_string()
    }
}
