mod events;

use crate::action::TimelineEdit;
use crate::ops::{Op, escape, set};
use anyhow::{Result, bail, ensure};
use serde_json::{Map, Value, json};
use std::path::Path;
use wprs_core::message::said;
use wprs_scene::MAX_TIMELINES;

const LIST: &str = "/scene/timelines";

fn bound(value: &Value, pointer: &str, id: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if map.get("timeline").and_then(Value::as_str) == Some(id) {
                out.push(format!("{pointer}/timeline"));
            }
            map.iter().for_each(|(k, v)| bound(v, &format!("{pointer}/{}", escape(k)), id, out));
        }
        Value::Array(list) => {
            list.iter().enumerate().for_each(|(i, v)| bound(v, &format!("{pointer}/{i}"), id, out))
        }
        _ => {}
    }
}

fn valid(id: &str) -> Result<()> {
    let fits = (1..=32).contains(&id.len())
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    ensure!(fits, said("editor.timeline_id", "a timeline id has 1 to 32 small letters, digits or _", &[]));
    Ok(())
}

fn number(line: &mut Map<String, Value>, key: &str, v: Option<f64>) -> Result<()> {
    let Some(v) = v else { return Ok(()) };
    ensure!(v.is_finite(), said("editor.timeline_number", "%1 of a timeline is a finite number", &[&key]));
    line.insert(key.into(), json!((v * 1000.0).round() / 1000.0));
    Ok(())
}

fn lengths(line: &mut Map<String, Value>, e: &TimelineEdit) -> Result<()> {
    let counted = e.fps.is_some() || e.frames.is_some();
    ensure!(
        !(counted && e.duration.is_some()),
        said("editor.timeline_length", "a timeline lasts seconds or frames, not both", &[])
    );
    if e.duration.is_some() {
        line.remove("fps");
        line.remove("frames");
    }
    if counted {
        line.remove("duration");
    }
    number(line, "duration", e.duration)?;
    number(line, "fps", e.fps)?;
    if let Some(n) = e.frames {
        line.insert("frames".into(), json!(n));
    }
    let timed = ["duration", "fps", "frames"].iter().any(|k| line.contains_key(*k));
    if !timed {
        line.insert("duration".into(), json!(5));
    }
    Ok(())
}

fn fields(line: &mut Map<String, Value>, e: &TimelineEdit) -> Result<()> {
    lengths(line, e)?;
    number(line, "speed", e.speed)?;
    number(line, "delay", e.delay)?;
    match e.wrap {
        Some(true) => drop(line.insert("wrap".into(), json!(true))),
        Some(false) => drop(line.remove("wrap")),
        None => {}
    }
    if let Some(r) = e.repeat {
        line.insert("loop".into(), json!(r));
        if line["loop"] != "repeat" && e.wrap != Some(true) {
            line.remove("wrap");
        }
    }
    if let Some(on) = e.autoplay {
        line.insert("autoplay".into(), json!(on));
    }
    match e.trigger.as_deref() {
        Some("") => drop(line.remove("trigger")),
        Some(t) => drop(line.insert("trigger".into(), json!(t))),
        None => {}
    }
    Ok(())
}

pub(crate) fn edit(dir: &Path, doc: &Value, e: &TimelineEdit) -> Result<Vec<Op>> {
    let list = doc.pointer(LIST).and_then(Value::as_array).cloned().unwrap_or_default();
    let id = e.id.as_str();
    let (i, mut line) = match (list.iter().position(|t| t["id"] == id), e.add) {
        (Some(_), true) => bail!(said("editor.timeline_taken", "there is already a timeline \"%1\"", &[&id])),
        (None, true) => {
            valid(id)?;
            ensure!(
                list.len() < MAX_TIMELINES,
                said("editor.timelines_most", "a scene has at most %1 timelines", &[&MAX_TIMELINES])
            );
            (list.len(), Map::from_iter([("id".to_string(), json!(id))]))
        }
        (None, false) => bail!(said("editor.there_no_timeline", "there is no timeline \"%1\"", &[&id])),
        (Some(i), false) => (i, list[i].as_object().cloned().unwrap_or_default()),
    };
    let mut users = Vec::new();
    bound(&doc["scene"], "/scene", id, &mut users);
    if e.remove {
        ensure!(!e.add, said("editor.timeline_add_remove", "add or remove a timeline, not both", &[]));
        let mut ops = vec![Op::Remove { path: format!("{LIST}/{i}") }];
        ops.extend(users.into_iter().map(|path| Op::Remove { path }));
        return crate::shaping::refused(dir, doc, ops);
    }
    fields(&mut line, e)?;
    events::events(&mut line, e)?;
    let mut follow = Vec::new();
    if let Some(to) = e.to.as_deref().filter(|t| *t != id) {
        valid(to)?;
        let taken = list.iter().any(|t| t["id"] == to);
        ensure!(!taken, said("editor.timeline_taken", "there is already a timeline \"%1\"", &[&to]));
        line.insert("id".into(), json!(to));
        follow = users.into_iter().map(|path| (path, json!(to))).collect();
    }
    let mut ops = match (e.add, list.is_empty()) {
        (true, true) => set(doc, LIST, json!([line])),
        (true, false) => vec![Op::Add { path: format!("{LIST}/-"), value: Value::Object(line) }],
        (false, _) => set(doc, &format!("{LIST}/{i}"), Value::Object(line)),
    };
    let mut staged = doc.clone();
    crate::ops::apply(&mut staged, &ops)?;
    for (path, to) in follow {
        ops.extend(set(&staged, &path, to));
    }
    crate::shaping::refused(dir, doc, ops)
}
