use serde_json::json;
use wprs_scene::{AudioFrame, Course, Fired, PointerState, Values};

pub struct Frame<'a> {
    pub time: f64,
    pub dt: f32,
    pub props: &'a Values,
    pub audio: &'a AudioFrame,
    pub pointer: &'a PointerState,
    pub actions: &'a [String],
    pub wall: f64,
    pub zone: &'a str,
    pub timelines: &'a [Course],
    pub fired: &'a [Fired],
}

impl Frame<'_> {
    pub fn json(&self) -> String {
        let (a, p) = (self.audio, self.pointer);
        let [x, y] = p.pos.unwrap_or(p.smooth);
        let (left, right, average) = (&a.left[..], &a.right[..], &a.average[..]);
        let audio = json!({"left": left, "right": right, "average": average, "bass": a.bass, "mid": a.mid,
            "treble": a.treble, "level": a.level, "beat": a.beat});
        let cursor = json!({"x": x, "y": y, "inside": p.inside, "buttons": p.buttons});
        let (actions, fired) = (self.actions, self.fired);
        let timelines: serde_json::Map<_, _> = (self.timelines.iter())
            .map(|c| {
                (
                    c.id.clone(),
                    json!({"time": c.time, "duration": c.duration, "playing": c.playing, "speed": c.speed}),
                )
            })
            .collect();
        json!({"time": self.time, "dt": self.dt, "props": self.props, "audio": audio, "cursor": cursor, "actions": actions,
            "wall": self.wall, "zone": self.zone, "timelines": timelines, "events": fired})
            .to_string()
    }
}
