use crate::bus;
use crate::editor::qobject::Editor;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::{Duration, Instant};
use wprs_core::edit::{self, Edit, Source};

const MERGE: Duration = Duration::from_millis(500);

pub fn gesture(action: &str, args: Option<&Value>) -> String {
    let changes = args.and_then(|a| a["changes"].as_array()).into_iter().flatten();
    let keys = changes.filter_map(Value::as_str).map(|c| c.split('=').next().unwrap_or(c));
    let keys: Vec<&str> = keys.collect();
    let target =
        args.map(|a| [&a["output"], &a["name"], &a["id"]].map(|v| v.as_str().unwrap_or("")).join(" "));
    format!("{action} {} {}", keys.join(" "), target.unwrap_or_default())
}

pub fn load(file: &Path, pixels: [u32; 2]) -> (Edit, Result<bool, String>) {
    let blank = Edit { source: Source { pixels, ..Source::default() }, ..Edit::default() };
    if !file.is_file() {
        return (blank, Err(format!("cannot find {}", file.display())));
    }
    match edit::load(file, &edit::dir()) {
        Ok(Some((_, mut edit))) => {
            let bytes = std::fs::metadata(file).map_or(0, |m| m.len());
            let real = wprs_render::image_size(file).map_or([0; 2], |(w, h)| [w, h]);
            let changed = edit.source.changed(bytes, real);
            edit.source.pixels = if edit.source.pixels == [0; 2] { pixels } else { edit.source.pixels };
            (edit, Ok(changed))
        }
        Ok(None) => Source::read(file, pixels)
            .map(|source| (Edit { source, ..Edit::default() }, Ok(false)))
            .unwrap_or_else(|e| (blank, Err(e.to_string()))),
        Err(e) => (blank, Err(e)),
    }
}

impl Editor {
    pub fn record(mut self: Pin<&mut Self>, before: &Edit, gesture: String) {
        let now = Instant::now();
        let mut this = self.as_mut().rust_mut();
        let merge = this.last.as_ref().is_some_and(|(g, at)| *g == gesture && now - *at < MERGE);
        let after = this.edit.clone();
        match merge {
            true => this.history.redo.clear(),
            false => this.history.record(before, &after),
        }
        this.last = Some((gesture, now));
    }

    fn walk(mut self: Pin<&mut Self>, back: bool) {
        if !self.ready {
            return;
        }
        let mut this = self.as_mut().rust_mut();
        let current = this.edit.clone();
        let found = if back { this.history.undo(&current) } else { this.history.redo(&current) };
        let Some(edit) = found else { return };
        (this.edit, this.last) = (edit, None);
        self.as_mut().set_error(QString::default());
        self.changed_edit();
    }

    pub fn undo(self: Pin<&mut Self>) {
        self.walk(true);
    }

    pub fn redo(self: Pin<&mut Self>) {
        self.walk(false);
    }

    pub fn compare(mut self: Pin<&mut Self>, on: bool) {
        self.as_mut().set_comparing(on);
        self.draw();
    }

    pub fn save_as(mut self: Pin<&mut Self>, file: &QString, quality: u32) {
        let out = crate::app::path(&file.to_string()).map(PathBuf::from).unwrap_or_default();
        let (edit, image, qt) = (self.edit.clone(), self.file.clone(), self.qt_thread());
        self.as_mut().set_exported(QString::default());
        bus::spawn(async move {
            let made = {
                let out = out.clone();
                tokio::task::spawn_blocking(move || crate::picture::export(&image, &edit, &out, quality))
                    .await
            };
            let made = made.map_err(anyhow::Error::from).and_then(|m| m);
            qt.queue(move |mut editor| match made {
                Ok(()) => editor.as_mut().set_exported(QString::from(&*out.to_string_lossy())),
                Err(e) => editor.as_mut().set_error(QString::from(&format!("{e:#}"))),
            })
            .ok();
        });
    }
}
