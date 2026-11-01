use crate::bus;
use crate::scene::qobject::SceneEditor;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::{Value, json};
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use wprs_editor::actions::visible;

const THROTTLE: Duration = Duration::from_millis(50);
const REFRESH: Duration = Duration::from_secs(30);
const EDGE: f64 = 1280.0;
static STREAMS: AtomicU32 = AtomicU32::new(0);

impl SceneEditor {
    pub fn attach(mut self: Pin<&mut Self>) {
        let Some(project) = self.project.as_ref() else { return };
        let size = &project.doc["scene"]["canvas"]["size"];
        let (w, h) =
            (size[0].as_f64().unwrap_or(1920.0).max(1.0), size[1].as_f64().unwrap_or(1080.0).max(1.0));
        let scale = (EDGE / w.max(h)).min(1.0);
        let pixels = (((w * scale).round() as u32).max(16), ((h * scale).round() as u32).max(16));
        if pixels == self.attached {
            return;
        }
        let output = format!("preview:{}-{}", std::process::id(), STREAMS.fetch_add(1, Ordering::Relaxed));
        let (old, qt) = (self.output.to_string(), self.qt_thread());
        self.as_mut().rust_mut().attached = pixels;
        self.as_mut().set_output(QString::from(&output));
        bus::spawn(async move {
            if !old.is_empty() {
                bus::call("Daemon1", "ReleaseStream", vec![json!(old)]).await.ok();
            }
            let reply =
                bus::call("Daemon1", "RequestStream", vec![json!(output), json!(pixels.0), json!(pixels.1)])
                    .await;
            let reply = reply.map(|r| serde_json::from_str::<Value>(&r).unwrap_or_default());
            qt.queue(move |mut editor| match reply {
                Ok(r) => {
                    editor.as_mut().set_node(r[0].as_u64().unwrap_or(0) as u32);
                    editor.as_mut().set_serial(r[1].as_f64().unwrap_or(0.0));
                    editor.send();
                }
                Err(e) => editor.fail(e.message),
            })
            .ok();
        });
    }

    pub fn cover(self: Pin<&mut Self>, covered: bool) {
        let output = self.output.to_string();
        if !output.is_empty() {
            bus::spawn(async move {
                bus::call("Daemon1", "SetOutputCovered", vec![json!(output), json!(covered)]).await.ok();
            });
        }
    }

    pub fn seek(mut self: Pin<&mut Self>, time: f64, playing: bool) {
        self.as_mut().set_time(time.max(0.0));
        self.as_mut().set_playing(playing);
        self.as_mut().refresh();
        self.send();
    }

    pub fn preview(mut self: Pin<&mut Self>, live: bool) {
        self.as_mut().set_live(live);
        self.send();
    }

    pub fn send(mut self: Pin<&mut Self>) {
        let Some(project) = self.project.as_ref() else { return };
        let streamed = !self.output.is_empty() && self.serial > 0.0;
        if !streamed && !self.live && !self.shown {
            return;
        }
        if self.sending {
            self.as_mut().rust_mut().queued = true;
            return;
        }
        let mut scene = visible(&project.doc);
        wprs_editor::rigs::preview(
            &mut scene,
            &serde_json::from_str(&self.look.to_string()).unwrap_or_default(),
        );
        let document = json!({"manifest": project.doc["manifest"], "scene": scene}).to_string();
        let dir = project.dir.to_string_lossy().into_owned();
        let time = if self.playing { -1.0 } else { self.time };
        let mut targets: Vec<(String, String)> = Vec::new();
        targets.extend(streamed.then(|| (self.output.to_string(), document.clone())));
        match (self.live, self.shown) {
            (true, _) => targets.push((String::new(), document)),
            (false, true) => targets.push((String::new(), String::new())),
            _ => {}
        }
        let (qt, live) = (self.qt_thread(), self.live);
        let mut this = self.as_mut().rust_mut();
        (this.sending, this.shown, this.sent) = (true, live, this.sent + 1);
        let sent = this.sent;
        bus::spawn(async move {
            let mut failed = None;
            for (output, document) in targets {
                let args = vec![json!(output), json!(dir), json!(document), json!(time)];
                if let Err(e) = bus::call("Daemon1", "PreviewScene", args).await {
                    failed = failed.or((!e.name.ends_with("InvalidArgs")).then_some(e.message));
                }
            }
            tokio::time::sleep(THROTTLE).await;
            qt.queue(move |mut editor| {
                editor.as_mut().rust_mut().sending = false;
                if let Some(message) = failed {
                    editor.as_mut().set_error(QString::from(&message));
                }
                match std::mem::take(&mut editor.as_mut().rust_mut().queued) {
                    true => editor.send(),
                    false => editor.keep(sent),
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
                if editor.sent == sent && !editor.sending && (editor.live || !editor.output.is_empty()) {
                    editor.send();
                }
            })
            .ok();
        });
    }
}
