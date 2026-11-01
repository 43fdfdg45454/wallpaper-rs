use super::Engine;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

pub(crate) type Steered = (u64, Vec<(PathBuf, u32)>);

impl Engine {
    pub(crate) fn steer(&mut self) -> Option<String> {
        let stage = self.stage.as_ref()?;
        let videos = stage.videos();
        let tapes = stage.tapes();
        let found = |asset: &str| videos.iter().find(|(id, _)| id == asset).map(|(_, path)| path);
        let mut h = DefaultHasher::new();
        for t in tapes {
            let wants = t.shown && t.playing && !t.ended && t.rate > 0.0;
            (found(&t.asset), t.jumps, wants, t.rate.to_bits(), t.repeat).hash(&mut h);
        }
        if std::mem::replace(&mut self.steered.0, h.finish()) == h.finish() {
            return None;
        }
        let mut list = Vec::new();
        for t in tapes {
            let Some(path) = found(&t.asset) else { continue };
            let last = self.steered.1.iter().position(|(p, _)| p == path);
            let seek = last.map_or(t.at > 0.05, |i| self.steered.1[i].1 != t.jumps);
            match last {
                Some(i) => self.steered.1[i].1 = t.jumps,
                None => self.steered.1.push((path.clone(), t.jumps)),
            }
            let playing = t.shown && t.playing && !t.ended && t.rate > 0.0;
            list.push(serde_json::json!({ "path": path, "playing": playing, "rate": t.rate.clamp(0.1, 4.0),
                "at": (t.at * 1000.0) as i64, "seek": seek, "loop": t.repeat }));
        }
        Some(serde_json::Value::Array(list).to_string())
    }
}
