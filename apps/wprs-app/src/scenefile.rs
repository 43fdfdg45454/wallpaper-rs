use crate::bus;
use crate::scene::SceneEditorRust;
use crate::scene::qobject::SceneEditor;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use serde_json::json;
use std::path::PathBuf;
use std::pin::Pin;
use wprs_editor::starters::slug;
use wprs_editor::{check, project};

pub fn projects_dir() -> PathBuf {
    if bus::checking() {
        return std::env::temp_dir().join(format!("wprs-app-check-{}", std::process::id()));
    }
    dirs::document_dir().or_else(dirs::home_dir).unwrap_or_default().join("wallpaper-rs")
}

impl SceneEditor {
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
        bus::spawn(async move {
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
