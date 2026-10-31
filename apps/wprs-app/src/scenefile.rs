use crate::bus;
use crate::scene::SceneEditorRust;
use crate::scene::qobject::SceneEditor;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use wprs_editor::starters::{default_id, locale, slug, starter};
use wprs_editor::{Project, check, package, project};

pub fn projects_dir() -> PathBuf {
    if bus::checking() {
        return std::env::temp_dir().join(format!("wprs-app-check-{}", std::process::id()));
    }
    dirs::document_dir().or_else(dirs::home_dir).unwrap_or_default().join("wallpaper-rs")
}

impl SceneEditor {
    pub fn fail(mut self: Pin<&mut Self>, error: impl Shown) {
        self.as_mut().set_error(QString::from(&error.shown()));
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
            Err(e) => self.fail(e),
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

    pub fn save(mut self: Pin<&mut Self>) {
        let saved = {
            let mut this = self.as_mut().rust_mut();
            let Some(project) = this.project.as_mut() else { return };
            project
                .save()
                .map(|_| (check::check_files(&project.dir, &project.doc), project::stamp(&project.doc)))
        };
        match saved {
            Ok((files, stamp)) => {
                (self.as_mut().rust_mut().files, self.as_mut().rust_mut().persisted) = (files, stamp);
                self.as_mut().set_error(QString::default());
                self.refresh();
            }
            Err(e) => self.fail(e),
        }
    }

    pub fn recover(mut self: Pin<&mut Self>, keep: bool) {
        let recovered = {
            let mut this = self.as_mut().rust_mut();
            let Some(project) = this.project.as_mut() else { return };
            project.recover(keep)
        };
        match recovered {
            Ok(()) => {
                self.as_mut().refresh();
                self.send();
            }
            Err(e) => self.fail(e),
        }
    }

    pub fn prune(mut self: Pin<&mut Self>) {
        let Some(project) = self.project.as_ref() else { return };
        let pruned = wprs_editor::assets::prune(&project.dir, &project.doc);
        self.as_mut().rust_mut().files = check::check_files(&project.dir, &project.doc);
        match pruned {
            Ok(gone) => self.as_mut().finished(QString::from(&gone.join("\n"))),
            Err(e) => self.as_mut().fail(e),
        }
        self.refresh();
    }

    pub fn autosave(mut self: Pin<&mut Self>) {
        let Some(project) = self.project.as_ref() else { return };
        let stamp = project::stamp(&project.doc);
        if stamp != self.persisted && project.persist().is_ok() {
            self.as_mut().rust_mut().persisted = stamp;
        }
    }

    pub fn folder_for(&self, name: &QString) -> QString {
        let name = slug(&name.to_string());
        let mut free =
            (1..1000).map(|n| projects_dir().join(if n == 1 { name.clone() } else { format!("{name}-{n}") }));
        let folder = free.find(|p| std::fs::read_dir(p).map_or(true, |mut d| d.next().is_none()));
        QString::from(folder.unwrap_or_default().to_string_lossy().as_ref())
    }

    pub fn projects(&self) -> QString {
        let found = std::fs::read_dir(projects_dir()).into_iter().flatten().flatten().filter_map(|entry| {
            let doc = project::read(&entry.path()).ok()?;
            let title = wprs_editor::meta::title(&doc, &wprs_core::locale::languages());
            Some(json!({"folder": entry.path(), "title": title}))
        });
        QString::from(&serde_json::to_string(&found.collect::<Vec<_>>()).unwrap_or_default())
    }
}

impl Drop for SceneEditorRust {
    fn drop(&mut self) {
        match self.project.as_ref() {
            Some(project) if project.dirty() || project.pending.is_some() => drop(project.persist()),
            Some(project) => drop(project.forget()),
            None => {}
        }
        let (output, shown) = (self.output.to_string(), self.shown);
        let dir = self.project.as_ref().map(|p| p.dir.to_string_lossy().into_owned()).unwrap_or_default();
        if output.is_empty() && !shown {
            return;
        }
        bus::farewell(async move {
            if shown {
                bus::call("Daemon1", "PreviewScene", vec![json!(""), json!(dir), json!(""), json!(0.0)])
                    .await
                    .ok();
            }
            if !output.is_empty() {
                bus::call("Daemon1", "ReleaseStream", vec![json!(output)]).await.ok();
            }
        });
    }
}

pub trait Shown {
    fn shown(&self) -> String;
}

impl Shown for anyhow::Error {
    fn shown(&self) -> String {
        wprs_editor::told(self)
    }
}

macro_rules! shown {
    ($($t:ty),*) => {$(impl Shown for $t { fn shown(&self) -> String { self.to_string() } })*};
}

shown!(String, &str, serde_json::Error);
