use crate::{Cover, Event, Sink};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

const EVENTS: [&str; 4] = ["fullscreen", "workspacev2", "focusedmon", "activespecial"];

pub(crate) fn watch(sink: Sink) {
    let Some(dir) = socket_dir() else { return };
    let spawned = std::thread::Builder::new().name("wprs-hyprland".into()).spawn(move || {
        if let Err(e) = follow(&dir, &sink) {
            tracing::warn!("stopped following Hyprland windows: {e}");
        }
    });
    if let Err(e) = spawned {
        tracing::warn!("cannot follow Hyprland windows: {e}");
    }
}

fn socket_dir() -> Option<PathBuf> {
    let signature = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").filter(|s| !s.is_empty())?;
    Some(PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?).join("hypr").join(signature))
}

fn follow(dir: &Path, sink: &Sink) -> std::io::Result<()> {
    let events = BufReader::new(UnixStream::connect(dir.join(".socket2.sock"))?);
    let mut known = HashMap::new();
    let mut report = || match monitors(dir) {
        Ok(monitors) => monitors.into_iter().for_each(|(name, covered)| {
            if known.insert(name.clone(), covered) != Some(covered) {
                sink(Event::Covered(name, Cover::Hyprland, covered));
            }
        }),
        Err(e) => tracing::debug!("cannot read the Hyprland monitors: {e}"),
    };
    report();
    for line in events.split(b'\n') {
        if EVENTS.contains(&String::from_utf8_lossy(&line?).split(">>").next().unwrap_or_default()) {
            report();
        }
    }
    Ok(())
}

fn monitors(dir: &Path) -> std::io::Result<Vec<(String, bool)>> {
    Ok(covered(&query(dir, "monitors")?, &query(dir, "workspaces")?))
}

fn query(dir: &Path, what: &str) -> std::io::Result<Value> {
    let mut socket = UnixStream::connect(dir.join(".socket.sock"))?;
    socket.write_all(format!("j/{what}").as_bytes())?;
    let mut reply = Vec::new();
    socket.read_to_end(&mut reply)?;
    serde_json::from_slice(&reply).map_err(std::io::Error::other)
}

fn covered(monitors: &Value, workspaces: &Value) -> Vec<(String, bool)> {
    let list = |v: &Value| v.as_array().cloned().unwrap_or_default();
    let full: Vec<i64> = (list(workspaces).iter())
        .filter(|w| w["hasfullscreen"].as_bool() == Some(true))
        .filter_map(|w| w["id"].as_i64())
        .collect();
    let shows_full = |w: &Value| w["id"].as_i64().is_some_and(|id| id != 0 && full.contains(&id));
    (list(monitors).iter())
        .filter_map(|m| {
            let name = m["name"].as_str()?.to_owned();
            Some((name, shows_full(&m["activeWorkspace"]) || shows_full(&m["specialWorkspace"])))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_monitor_is_covered_when_its_visible_workspace_has_a_fullscreen_window() {
        let monitors = json!([
            { "name": "DP-1", "activeWorkspace": { "id": 1 }, "specialWorkspace": { "id": 0 } },
            { "name": "DP-2", "activeWorkspace": { "id": 2 }, "specialWorkspace": { "id": -98 } },
            { "name": "HDMI-A-1", "activeWorkspace": { "id": 3 }, "specialWorkspace": { "id": 0 } },
        ]);
        let workspaces = json!([
            { "id": 1, "hasfullscreen": true }, { "id": 2, "hasfullscreen": false },
            { "id": 3, "hasfullscreen": false }, { "id": 4, "hasfullscreen": true },
            { "id": -98, "hasfullscreen": true },
        ]);
        let expected = [("DP-1", true), ("DP-2", true), ("HDMI-A-1", false)].map(|(n, c)| (n.to_owned(), c));
        assert_eq!(covered(&monitors, &workspaces), expected);
        assert!(covered(&json!({}), &workspaces).is_empty());
    }
}
