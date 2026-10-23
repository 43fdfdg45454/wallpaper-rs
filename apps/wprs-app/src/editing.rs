use crate::bus;
use crate::editor::EditorRust;
use crate::editor::qobject::Editor;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::{Map, Value, json};
use std::pin::Pin;
use std::time::Duration;
use wprs_core::edit::{Action, Edit, Kind, Parallax, spline};

const LEVELS: [&str; 6] = ["ev", "temperature", "tint", "contrast", "saturation", "vibrance"];
const CHANNELS: [&str; 4] = ["rgb", "r", "g", "b"];
const THROTTLE: Duration = Duration::from_millis(50);
const REFRESH: Duration = Duration::from_secs(30);

fn curve(edit: &Edit, channel: &str) -> Value {
    let points = match edit.ops.iter().find(|op| op.id == "cu").map(|op| &op.kind) {
        Some(Kind::Curves { rgb, r, g, b }) => {
            [rgb, r, g, b][CHANNELS.iter().position(|c| *c == channel).unwrap_or(0)].clone()
        }
        _ => Vec::new(),
    };
    let points = if points.is_empty() { vec![[0.0, 0.0], [1.0, 1.0]] } else { points };
    let at = spline(&points);
    let samples: Vec<f32> = (0..=64).map(|i| at(i as f32 / 64.0).clamp(0.0, 1.0)).collect();
    json!({"points": points, "samples": samples})
}

pub fn describe(edit: &Edit) -> String {
    let levels: Map<String, Value> =
        LEVELS.iter().map(|k| (k.to_string(), edit.get(k).unwrap_or_default())).collect();
    let mut probe = edit.clone();
    let fresh = [
        Action::Blur { radius: None, mask: None, id: None },
        Action::Vignette { amount: None, midpoint: None, roundness: None, feather: None, id: None },
        Action::Overlay { color: None, blend: None, opacity: None, mask: None, id: None },
    ];
    for action in fresh {
        probe.act(action).ok();
    }
    let tools: Map<String, Value> = probe
        .ops
        .iter()
        .filter(|op| ["bl", "vi", "ov"].contains(&op.id.as_str()))
        .map(|op| {
            let mut tool = serde_json::to_value(op).unwrap_or_default();
            tool["opacity"] = json!(op.opacity.unwrap_or(1.0));
            (op.id.clone(), tool)
        })
        .collect();
    let masks: Map<String, Value> = edit
        .masks
        .iter()
        .filter_map(|(name, mask)| {
            let spec = mask.spec()?;
            let (kind, values) = spec.split_once(':')?;
            let values: Vec<f64> = values.split(',').filter_map(|v| v.parse().ok()).collect();
            let invert = serde_json::to_value(mask).ok()?["invert"].clone();
            Some((name.clone(), json!({"kind": kind, "values": values, "invert": invert})))
        })
        .collect();
    let curves: Map<String, Value> = CHANNELS.iter().map(|c| (c.to_string(), curve(edit, c))).collect();
    let depth = edit.depth.clone().unwrap_or_default();
    let parallax = edit.parallax.clone().unwrap_or(Parallax { enabled: false, ..Parallax::default() });
    json!({"levels": levels, "tools": tools, "curves": curves, "masks": masks, "depth": depth, "parallax": parallax})
    .to_string()
}

impl Editor {
    pub fn preview(mut self: Pin<&mut Self>, stop: bool) {
        self.as_mut().set_live(!stop);
        self.send();
    }

    pub fn send(mut self: Pin<&mut Self>) {
        if !self.live && !self.shown {
            return;
        }
        if self.sending {
            self.as_mut().rust_mut().queued = true;
            return;
        }
        let json = if self.live { self.edit.to_json() } else { String::new() };
        let (path, qt) = (self.file.to_string_lossy().into_owned(), self.qt_thread());
        let mut this = self.as_mut().rust_mut();
        (this.sending, this.shown, this.sent) = (true, this.live, this.sent + 1);
        let sent = this.sent;
        bus::spawn(async move {
            let reply = bus::call("Daemon1", "PreviewEdit", vec![json!(path), json!(json)]).await;
            tokio::time::sleep(THROTTLE).await;
            qt.queue(move |mut editor| {
                editor.as_mut().rust_mut().sending = false;
                if let Err(e) = reply {
                    editor.as_mut().set_error(QString::from(&e.message));
                }
                match std::mem::take(&mut editor.as_mut().rust_mut().queued) {
                    true => editor.send(),
                    false if editor.live => editor.keep(sent),
                    false => {}
                }
            })
            .ok();
        });
    }

    fn keep(self: Pin<&mut Self>, sent: u64) {
        let qt = self.qt_thread();
        bus::spawn(async move {
            tokio::time::sleep(REFRESH).await;
            qt.queue(move |editor| {
                if editor.live && editor.sent == sent && !editor.sending {
                    editor.send();
                }
            })
            .ok();
        });
    }
}

impl Drop for EditorRust {
    fn drop(&mut self) {
        self.drawn.take().map(std::fs::remove_file);
        if self.shown {
            let path = self.file.to_string_lossy().into_owned();
            bus::spawn(async move {
                bus::call("Daemon1", "PreviewEdit", vec![json!(path), json!("")]).await.ok();
            });
        }
    }
}
