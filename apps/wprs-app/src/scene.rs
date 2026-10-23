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
        #[qproperty(QString, folder)]
        #[qproperty(QString, view)]
        #[qproperty(QString, fields)]
        #[qproperty(QString, selected)]
        #[qproperty(QString, error)]
        #[qproperty(QString, output)]
        #[qproperty(u32, node)]
        #[qproperty(f64, serial)]
        #[qproperty(f64, time)]
        #[qproperty(bool, playing)]
        #[qproperty(bool, ready)]
        #[qproperty(bool, dirty)]
        #[qproperty(bool, live)]
        #[qproperty(bool, busy)]
        type SceneEditor = super::SceneEditorRust;

        #[qinvokable]
        fn create(self: Pin<&mut SceneEditor>, folder: &QString, starter: &QString, title: &QString);
        #[qinvokable]
        fn open(self: Pin<&mut SceneEditor>, folder: &QString, package: &QString);
        #[qinvokable]
        fn run(self: Pin<&mut SceneEditor>, action: &QString, args: &QString, gesture: bool);
        #[qinvokable]
        fn select(self: Pin<&mut SceneEditor>, pointer: &QString);
        #[qinvokable]
        fn inspect(self: &SceneEditor, pointer: &QString) -> QString;
        #[qinvokable]
        fn count(self: &SceneEditor, pointer: &QString) -> i32;
        #[qinvokable]
        fn value(self: &SceneEditor, pointer: &QString) -> QString;
        #[qinvokable]
        fn save(self: Pin<&mut SceneEditor>);
        #[qinvokable]
        fn recover(self: Pin<&mut SceneEditor>, keep: bool);
        #[qinvokable]
        fn prune(self: Pin<&mut SceneEditor>);
        #[qinvokable]
        fn autosave(self: Pin<&mut SceneEditor>);
        #[qinvokable]
        fn build(self: Pin<&mut SceneEditor>, out: &QString, key: &QString, password: &QString);
        #[qinvokable]
        fn apply(self: Pin<&mut SceneEditor>, output: &QString);
        #[qinvokable]
        fn seek(self: Pin<&mut SceneEditor>, time: f64, playing: bool);
        #[qinvokable]
        fn attach(self: Pin<&mut SceneEditor>);
        #[qinvokable]
        fn cover(self: Pin<&mut SceneEditor>, covered: bool);
        #[qinvokable]
        fn preview(self: Pin<&mut SceneEditor>, live: bool);
        #[qinvokable]
        fn folder_for(self: &SceneEditor, name: &QString) -> QString;
        #[qinvokable]
        fn projects(self: &SceneEditor) -> QString;
        #[qsignal]
        fn finished(self: Pin<&mut SceneEditor>, message: QString);
    }

    impl cxx_qt::Threading for SceneEditor {}
}

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use qobject::SceneEditor;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use wprs_editor::starters::{default_id, locale, starter};
use wprs_editor::{Action, Problem, Project, check, inspect, outline, package, project};

#[derive(Default)]
pub struct SceneEditorRust {
    pub folder: QString,
    pub view: QString,
    pub fields: QString,
    pub selected: QString,
    pub error: QString,
    pub output: QString,
    pub node: u32,
    pub serial: f64,
    pub time: f64,
    pub playing: bool,
    pub ready: bool,
    pub dirty: bool,
    pub live: bool,
    pub busy: bool,
    pub project: Option<Project>,
    pub imports: Vec<Action>,
    pub files: Vec<Problem>,
    pub persisted: String,
    pub sending: bool,
    pub queued: bool,
    pub sent: u64,
    pub shown: bool,
    pub attached: (u32, u32),
}

impl SceneEditor {
    pub fn fail(mut self: Pin<&mut Self>, error: impl ToString) {
        self.as_mut().set_error(QString::from(&error.to_string()));
    }

    fn loaded(mut self: Pin<&mut Self>, project: anyhow::Result<Project>) {
        match project {
            Ok(project) => {
                let folder = project.dir.to_string_lossy().into_owned();
                self.as_mut().rust_mut().files = check::check_files(&project.dir, &project.doc);
                self.as_mut().rust_mut().persisted = project::stamp(&project.doc);
                self.as_mut().rust_mut().project = Some(project);
                self.as_mut().set_folder(QString::from(&folder));
                self.as_mut().set_selected(QString::from("/scene/canvas"));
                self.as_mut().set_error(QString::default());
                self.as_mut().set_ready(true);
                self.as_mut().attach();
                self.refresh();
            }
            Err(e) => self.fail(format!("{e:#}")),
        }
    }

    pub fn create(self: Pin<&mut Self>, folder: &QString, starter_name: &QString, title: &QString) {
        let dir = PathBuf::from(folder.to_string());
        let name = dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let title = Some(title.to_string()).filter(|t| !t.trim().is_empty()).unwrap_or_else(|| name.clone());
        let made = starter(&starter_name.to_string(), &default_id(&name), &title, &locale());
        self.loaded(made.and_then(|doc| Project::create(&dir, doc)));
    }

    pub fn open(self: Pin<&mut Self>, folder: &QString, package_file: &QString) {
        let dir = PathBuf::from(folder.to_string());
        let extracted = match package_file.to_string() {
            file if file.is_empty() => Ok(()),
            file => package::extract(Path::new(&file), &dir).map(drop),
        };
        self.loaded(extracted.and_then(|_| Project::open(&dir)));
    }

    pub fn run(mut self: Pin<&mut Self>, action: &QString, args: &QString, gesture: bool) {
        let args = serde_json::from_str::<Value>(&args.to_string()).unwrap_or(json!({}));
        let action = match serde_json::from_value::<Action>(json!({ action.to_string(): args })) {
            Ok(action @ (Action::Import { .. } | Action::Thumbnail { .. })) => return self.import(action),
            Ok(action) => action,
            Err(e) => return self.fail(e),
        };
        let done = {
            let mut this = self.as_mut().rust_mut();
            let Some(project) = this.project.as_mut() else { return };
            if gesture { project.drag(&action) } else { project.act(&action) }
        };
        match done {
            Ok(_) => self.changed(),
            Err(e) => self.fail(e),
        }
    }

    pub fn changed(mut self: Pin<&mut Self>) {
        self.as_mut().set_error(QString::default());
        self.as_mut().refresh();
        self.as_mut().attach();
        self.send();
    }

    pub fn select(mut self: Pin<&mut Self>, pointer: &QString) {
        self.as_mut().set_selected(pointer.clone());
        self.show_fields();
    }

    pub fn inspect(&self, pointer: &QString) -> QString {
        let fields =
            self.project.as_ref().map(|p| inspect::fields_at(&p.doc, &pointer.to_string(), self.time));
        QString::from(&serde_json::to_string(&fields.unwrap_or_default()).unwrap_or_default())
    }

    pub fn value(&self, pointer: &QString) -> QString {
        let found = self.project.as_ref().and_then(|p| p.doc.pointer(&pointer.to_string()).cloned());
        QString::from(&found.unwrap_or_default().to_string())
    }

    pub fn count(&self, pointer: &QString) -> i32 {
        let list =
            self.project.as_ref().and_then(|p| p.doc.pointer(&pointer.to_string())?.as_array().map(Vec::len));
        list.map_or(-1, |n| n as i32)
    }

    pub fn show_fields(mut self: Pin<&mut Self>) {
        let fields = self.inspect(&self.selected.clone());
        self.as_mut().set_fields(fields);
    }

    pub fn refresh(mut self: Pin<&mut Self>) {
        let Some(project) = self.project.as_ref() else { return };
        let view = outline::outline(project, self.time, &self.files).to_string();
        let dirty = project.dirty();
        self.as_mut().set_view(QString::from(&view));
        self.as_mut().set_dirty(dirty);
        self.show_fields();
    }
}
