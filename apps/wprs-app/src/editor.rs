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
        #[qproperty(bool, changed)]
        #[qproperty(bool, can_undo)]
        #[qproperty(bool, can_redo)]
        #[qproperty(bool, comparing)]
        #[qproperty(QString, exported)]
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
        #[qinvokable]
        fn undo(self: Pin<&mut Editor>);
        #[qinvokable]
        fn redo(self: Pin<&mut Editor>);
        #[qinvokable]
        fn compare(self: Pin<&mut Editor>, on: bool);
        #[qinvokable]
        fn save_as(self: Pin<&mut Editor>, file: &QString, quality: u32);
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
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;
use wprs_core::edit::{self, Action, Edit, History};

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
    pub changed: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub comparing: bool,
    pub exported: QString,
    pub file: PathBuf,
    pub history: History,
    pub last: Option<(String, Instant)>,
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

impl Editor {
    pub fn open(mut self: Pin<&mut Self>, path: &QString, width: u32, height: u32) {
        let (file, qt) = (PathBuf::from(path.to_string()), self.qt_thread());
        self.as_mut().rust_mut().file = file.clone();
        bus::spawn(async move {
            let missing = !file.is_file();
            let loaded = tokio::task::spawn_blocking(move || {
                (crate::editsteps::load(&file, [width, height]), History::load(&file))
            })
            .await;
            let ((edit, result), history) =
                loaded.unwrap_or_else(|e| ((Edit::default(), Err(e.to_string())), History::default()));
            qt.queue(move |mut editor| {
                let mut this = editor.as_mut().rust_mut();
                (this.saved, this.edit, this.history) = (edit.clone(), edit, history);
                let loaded = result.is_ok();
                editor.as_mut().set_changed(result.as_ref().is_ok_and(|c| *c));
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
        let gesture = crate::editsteps::gesture(&action.to_string(), args.as_ref().ok());
        let action =
            args.and_then(|args| serde_json::from_value::<Action>(json!({ action.to_string(): args })));
        let before = self.edit.clone();
        let done = match action {
            Ok(action) => self.as_mut().rust_mut().edit.act(action).map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        let failed = done.err();
        self.as_mut().set_error(QString::from(failed.as_deref().unwrap_or_default()));
        if failed.is_none() {
            self.as_mut().record(&before, gesture);
            self.changed_edit();
        }
    }

    pub fn changed_edit(mut self: Pin<&mut Self>) {
        self.as_mut().show();
        self.as_mut().draw();
        self.send();
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
            let mut this = self.as_mut().rust_mut();
            this.saved = edit;
            let file = this.file.clone();
            let saved = this.history.save(&file);
            saved.unwrap_or_else(|e| eprintln!("wprs-app: the undo history cannot be kept: {e}"));
        }
        self.as_mut().set_error(QString::from(done.err().unwrap_or_default().as_str()));
        self.show();
    }

    fn show(mut self: Pin<&mut Self>) {
        let (json, values, dirty) = (self.edit.to_json(), describe(&self.edit), self.edit != self.saved);
        let (undo, redo) = (!self.history.undo.is_empty(), !self.history.redo.is_empty());
        self.as_mut().set_json(QString::from(&json));
        self.as_mut().set_values(QString::from(&values));
        self.as_mut().set_can_undo(undo);
        self.as_mut().set_can_redo(redo);
        self.set_dirty(dirty);
    }
}
