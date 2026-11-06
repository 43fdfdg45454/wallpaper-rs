use crate::action::TimelineEdit;
use anyhow::{Result, bail, ensure};
use serde_json::{Map, Value, json};
use wprs_core::message::said;
use wprs_scene::MAX_EVENTS;

fn length(line: &Map<String, Value>) -> f64 {
    let counted = line.get("frames").and_then(Value::as_f64).zip(line.get("fps").and_then(Value::as_f64));
    counted.map_or_else(
        || line.get("duration").and_then(Value::as_f64).unwrap_or(0.0),
        |(n, fps)| n / fps.max(1.0),
    )
}

fn named(name: &str) -> Result<()> {
    let fits = (1..=32).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    ensure!(fits, said("editor.event_name", "an event name has 1 to 32 small letters, digits or _", &[]));
    Ok(())
}

fn picked(list: &[Value], spec: &str) -> Vec<usize> {
    let (name, at) = match spec.rsplit_once('@') {
        Some((name, t)) => (name, t.parse::<f64>().ok()),
        None => (spec, None),
    };
    let hit = |e: &Value| {
        e["name"] == name && at.is_none_or(|t| e["t"].as_f64().is_some_and(|v| (v - t).abs() < 5e-4))
    };
    list.iter().enumerate().filter(|(_, e)| hit(e)).map(|(i, _)| i).collect()
}

pub(super) fn events(line: &mut Map<String, Value>, e: &TimelineEdit) -> Result<()> {
    let touched = e.clear_events || !e.drop_events.is_empty() || !e.add_events.is_empty();
    if !touched {
        return Ok(());
    }
    let mut list = if e.clear_events {
        Vec::new()
    } else {
        line.get("events").and_then(Value::as_array).cloned().unwrap_or_default()
    };
    for spec in &e.drop_events {
        let hits = picked(&list, spec);
        if hits.is_empty() {
            bail!(said("editor.no_event", "the timeline has no event %1", &[spec]));
        }
        hits.into_iter().rev().for_each(|i| drop(list.remove(i)));
    }
    let length = length(line);
    for event in &e.add_events {
        named(&event.name)?;
        let inside = event.t.is_finite() && (0.0..=length).contains(&event.t);
        ensure!(
            inside,
            said("editor.event_time", "an event fires from 0 s to the end of the timeline, %1 s", &[&length])
        );
        let t = (event.t * 1000.0).round() / 1000.0;
        let twin = list.iter().any(|v| v["name"] == event.name.as_str() && v["t"].as_f64() == Some(t));
        ensure!(!twin, said("editor.event_twice", "the event %1 is already at %2 s", &[&event.name, &t]));
        list.push(json!({ "t": t, "name": event.name }));
    }
    ensure!(
        list.len() <= MAX_EVENTS,
        said("editor.events_most", "a timeline has at most %1 events", &[&MAX_EVENTS])
    );
    list.sort_by(|a, b| a["t"].as_f64().unwrap_or(0.0).total_cmp(&b["t"].as_f64().unwrap_or(0.0)));
    match list.is_empty() {
        true => drop(line.remove("events")),
        false => drop(line.insert("events".into(), Value::Array(list))),
    }
    Ok(())
}
