#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, picture)]
        #[qproperty(bool, whole)]
        #[qproperty(QString, trouble)]
        #[qproperty(QString, json)]
        #[qproperty(QString, values)]
        #[qproperty(QString, error)]
        #[qproperty(bool, ready)]
        #[qproperty(bool, dirty)]
        #[qproperty(bool, live)]
        type Editor = super::EditorRust;

        #[qinvokable]
        fn open(self: Pin<&mut Editor>, path: &QString, width: u32, height: u32);
        #[qinvokable]
        fn run(self: Pin<&mut Editor>, action: &QString, args: &QString);
        #[qinvokable]
        fn save(self: Pin<&mut Editor>);
        #[qinvokable]
        fn preview(self: Pin<&mut Editor>, stop: bool);
        #[qinvokable]
        fn view(self: Pin<&mut Editor>, whole: bool);
        #[qinvokable]
        fn refresh(self: Pin<&mut Editor>);
    }

    impl cxx_qt::Threading for Editor {}
}

use crate::bus;
use crate::editing::describe;
use crate::picture::Picture;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use qobject::Editor;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use wprs_core::edit::{self, Action, Edit, Source};

#[derive(Default)]
pub struct EditorRust {
    pub picture: QString,
    pub whole: bool,
    pub trouble: QString,
    pub json: QString,
    pub values: QString,
    pub error: QString,
    pub ready: bool,
    pub dirty: bool,
    pub live: bool,
    pub file: PathBuf,
    pub edit: Edit,
    pub saved: Edit,
    pub sending: bool,
    pub queued: bool,
    pub shown: bool,
    pub sent: u64,
    pub art: Option<Arc<Picture>>,
    pub wanted: bool,
    pub drawing: bool,
    pub redraw: bool,
    pub drawn: Option<PathBuf>,
}

fn load(file: &Path, pixels: [u32; 2]) -> (Edit, Result<(), String>) {
    let blank = Edit { source: Source { pixels, ..Source::default() }, ..Edit::default() };
    if !file.is_file() {
        return (blank, Err(format!("cannot find {}", file.display())));
    }
    match edit::load(file, &edit::dir()) {
        Ok(Some((_, mut edit))) => {
            edit.source.pixels = if edit.source.pixels == [0; 2] { pixels } else { edit.source.pixels };
            (edit, Ok(()))
        }
        Ok(None) => Source::read(file, pixels)
            .map(|source| (Edit { source, ..Edit::default() }, Ok(())))
            .unwrap_or_else(|e| (blank, Err(e.to_string()))),
        Err(e) => (blank, Err(e)),
    }
}

impl Editor {
    pub fn open(mut self: Pin<&mut Self>, path: &QString, width: u32, height: u32) {
        let (file, qt) = (PathBuf::from(path.to_string()), self.qt_thread());
        self.as_mut().rust_mut().file = file.clone();
        bus::spawn(async move {
            let missing = !file.is_file();
            let loaded = tokio::task::spawn_blocking(move || load(&file, [width, height])).await;
            let (edit, result) = loaded.unwrap_or_else(|e| (Edit::default(), Err(e.to_string())));
            qt.queue(move |mut editor| {
                let mut this = editor.as_mut().rust_mut();
                (this.saved, this.edit) = (edit.clone(), edit);
                let loaded = result.is_ok();
                editor.as_mut().set_error(QString::from(result.err().unwrap_or_default().as_str()));
                editor.as_mut().set_ready(loaded || missing);
                editor.as_mut().show();
                if loaded {
                    editor.paint();
                }
            })
            .ok();
        });
    }

    pub fn run(mut self: Pin<&mut Self>, action: &QString, args: &QString) {
        if !self.ready {
            return;
        }
        let args = serde_json::from_str::<Value>(&args.to_string());
        let action =
            args.and_then(|args| serde_json::from_value::<Action>(json!({ action.to_string(): args })));
        let done = match action {
            Ok(action) => self.as_mut().rust_mut().edit.act(action).map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        let failed = done.err();
        self.as_mut().set_error(QString::from(failed.as_deref().unwrap_or_default()));
        if failed.is_none() {
            self.as_mut().show();
            self.as_mut().draw();
            self.send();
        }
    }

    pub fn save(mut self: Pin<&mut Self>) {
        if !self.ready {
            return;
        }
        let done = match self.file.is_file() {
            true => edit::commit(&self.edit, &self.file, &edit::dir()).map(drop),
            false => Err(format!("cannot find {}", self.file.display())),
        };
        if done.is_ok() {
            let edit = self.edit.clone();
            self.as_mut().rust_mut().saved = edit;
        }
        self.as_mut().set_error(QString::from(done.err().unwrap_or_default().as_str()));
        self.show();
    }

    fn show(mut self: Pin<&mut Self>) {
        let (json, values, dirty) = (self.edit.to_json(), describe(&self.edit), self.edit != self.saved);
        self.as_mut().set_json(QString::from(&json));
        self.as_mut().set_values(QString::from(&values));
        self.set_dirty(dirty);
    }
}
