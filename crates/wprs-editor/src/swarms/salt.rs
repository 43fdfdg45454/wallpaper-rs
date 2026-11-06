use crate::ops::escape;
use serde_json::Value;
use std::sync::OnceLock;

pub fn preview_seed() -> u64 {
    static SEED: OnceLock<u64> = OnceLock::new();
    *SEED.get_or_init(|| {
        let now = std::time::UNIX_EPOCH.elapsed().unwrap_or_default();
        (now.as_nanos() as u64 ^ (now.as_secs() << 32) ^ u64::from(std::process::id())).max(1)
    })
}

pub fn pin_seed(scene: &mut Value) {
    if let Some(clock) = scene.get_mut("clock").and_then(Value::as_object_mut)
        && clock.get("seed").is_some_and(Value::is_null)
    {
        clock.insert("seed".into(), preview_seed().into());
    }
}

pub(super) fn clock_seed(doc: &Value) -> u64 {
    match doc.pointer("/scene/clock/seed") {
        None => 0,
        Some(Value::Null) => preview_seed(),
        Some(v) => v.as_u64().unwrap_or(0),
    }
}

fn place(doc: &Value, system: &str, level: usize) -> Option<(usize, usize)> {
    if super::handles::drawn_by(doc, system).is_some() {
        return Some((level, 0));
    }
    let all = doc.pointer("/scene/particles")?.as_object()?;
    all.iter().filter(|_| level < wprs_scene::MAX_GENERATIONS).find_map(|(host, h)| {
        let kids = h["children"].as_array()?;
        let link = kids.iter().position(|c| c["system"] == system)?;
        place(doc, host, level + 1).map(|(depth, _)| (depth, link))
    })
}

pub(super) fn salt(doc: &Value, system: &str) -> (u32, u64) {
    let clock = clock_seed(doc);
    let own = doc.pointer(&format!("/scene/particles/{}/seed", escape(system))).and_then(Value::as_u64);
    let (depth, link) = place(doc, system, 0).unwrap_or((0, 0));
    (wprs_scene::particle_salt(own.unwrap_or(0), clock, depth, link), clock)
}
