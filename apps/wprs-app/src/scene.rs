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
        #[qproperty(QString, look)]
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
        fn condition(self: &SceneEditor, pointer: &QString, text: &QString) -> QString;
        #[qinvokable]
        fn scripts(self: &SceneEditor, id: &QString) -> QString;
        #[qinvokable]
        fn rig(self: &SceneEditor, id: &QString, query: &QString) -> QString;
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
use std::pin::Pin;
use wprs_editor::{Action, Problem, Project, inspect, outline, references};

#[derive(Default)]
pub struct SceneEditorRust {
    pub folder: QString,
    pub view: QString,
    pub fields: QString,
    pub selected: QString,
    pub error: QString,
    pub output: QString,
    pub look: QString,
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
        let fields = self.project.as_ref().map(|p| {
            inspect::fields_from(Some(&p.dir), &p.doc, &pointer.to_string(), self.time, &p.effects())
        });
        QString::from(&serde_json::to_string(&fields.unwrap_or_default()).unwrap_or_default())
    }

    pub fn value(&self, pointer: &QString) -> QString {
        let found = self.project.as_ref().and_then(|p| p.doc.pointer(&pointer.to_string()).cloned());
        QString::from(&found.unwrap_or_default().to_string())
    }

    pub fn condition(&self, pointer: &QString, text: &QString) -> QString {
        let doc = self.project.as_ref().map(|p| &p.doc);
        let found =
            doc.and_then(|d| wprs_editor::settings::condition(d, &pointer.to_string(), &text.to_string()));
        QString::from(&found.unwrap_or_default())
    }

    pub fn scripts(&self, id: &QString) -> QString {
        let found = self.project.as_ref().map(|p| references::scripts(&p.dir, &p.doc, &id.to_string()));
        QString::from(&serde_json::to_string(&found.unwrap_or_default()).unwrap_or_default())
    }

    pub fn rig(&self, id: &QString, query: &QString) -> QString {
        let q = serde_json::from_str(&query.to_string()).unwrap_or_default();
        let found =
            self.project.as_ref().map(|p| wprs_editor::rigs::query(&p.dir, &p.doc, &id.to_string(), &q));
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
