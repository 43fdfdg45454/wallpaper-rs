use crate::bus::{self, Event};
use crate::scene::qobject::SceneEditor;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use wprs_editor::{Action, assets, check, package, previews};

type Done<T> = Box<dyn FnOnce(Pin<&mut SceneEditor>, T) + Send>;

impl SceneEditor {
    fn work<T: Send + 'static>(
        mut self: Pin<&mut Self>,
        job: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
        done: Done<T>,
    ) {
        self.as_mut().set_busy(true);
        let qt = self.qt_thread();
        bus::spawn(async move {
            let result = tokio::task::spawn_blocking(job).await.map_err(anyhow::Error::from).and_then(|r| r);
            qt.queue(move |mut editor| {
                editor.as_mut().set_busy(false);
                match result {
                    Ok(value) => done(editor.as_mut(), value),
                    Err(e) => editor.as_mut().fail(format!("{e:#}")),
                }
                editor.next();
            })
            .ok();
        });
    }

    fn next(mut self: Pin<&mut Self>) {
        if self.busy || self.imports.is_empty() {
            return;
        }
        let action = self.as_mut().rust_mut().imports.remove(0);
        let Some(project) = self.project.as_ref() else { return };
        let (dir, doc) = (project.dir.clone(), project.doc.clone());
        let label = action.name();
        let job = move || match action {
            Action::Thumbnail { file, animated } => {
                Ok((None, previews::store(&dir, &doc, &file, animated)?, animated))
            }
            Action::Import { file, id } => {
                let (id, path) = assets::import(&dir, &doc, &file, id.as_deref())?;
                Ok((Some(id), path, false))
            }
            other => Err(anyhow::anyhow!("{} is not a file action", other.name())),
        };
        self.work(
            job,
            Box::new(move |mut editor, (id, path, animated)| {
                let recorded = {
                    let mut this = editor.as_mut().rust_mut();
                    let Some(project) = this.project.as_mut() else { return };
                    let ops = match &id {
                        Some(id) => assets::adopt(&project.doc, id, &path),
                        None => previews::adopt(&project.doc, animated, &path),
                    };
                    ops.and_then(|ops| project.record(&label, ops))
                };
                match recorded {
                    Ok(_) => {
                        editor.as_mut().rescan();
                        editor.changed();
                    }
                    Err(e) => editor.fail(e),
                }
            }),
        );
    }

    pub fn import(mut self: Pin<&mut Self>, action: Action) {
        self.as_mut().rust_mut().imports.push(action);
        self.next();
    }

    pub fn rescan(mut self: Pin<&mut Self>) {
        let files = self.project.as_ref().map(|p| check::check_files(&p.dir, &p.doc)).unwrap_or_default();
        self.as_mut().rust_mut().files = files;
        self.refresh();
    }

    fn saved(mut self: Pin<&mut Self>) -> Option<(PathBuf, Value)> {
        if self.busy {
            return None;
        }
        let project = self.as_mut().rust_mut().project.as_mut().map(|p| (p.dirty(), p.settle()));
        match project? {
            (true, _) => self.as_mut().fail("save the scene first"),
            (_, Err(e)) => self.as_mut().fail(format!("{e:#}")),
            (_, Ok(settled)) => {
                if settled {
                    self.as_mut().refresh();
                }
                let project = self.project.as_ref()?;
                return Some((project.dir.clone(), project.doc.clone()));
            }
        }
        None
    }

    pub fn build(mut self: Pin<&mut Self>, out: &QString, key: &QString, password: &QString) {
        let Some((dir, doc)) = self.as_mut().saved() else { return };
        let out = Some(out.to_string()).filter(|o| !o.is_empty()).map(PathBuf::from);
        let (key, password) = (key.to_string(), Some(password.to_string()).filter(|p| !p.is_empty()));
        let job = move || {
            let out = out.map_or_else(|| package::output(&dir, &doc), Ok)?;
            let signer = (!key.is_empty()).then(|| package::signer(Path::new(&key), password)).transpose()?;
            let (sha256, _) = package::build_saved(&dir, &doc, &out, signer.as_ref())?;
            Ok(format!("{}\n{sha256}", out.display()))
        };
        self.work(
            job,
            Box::new(|mut editor, message| {
                editor.as_mut().rescan();
                editor.finished(QString::from(&message));
            }),
        );
    }

    pub fn apply(mut self: Pin<&mut Self>, output: &QString) {
        let Some((dir, doc)) = self.as_mut().saved() else { return };
        let output = output.to_string();
        let job = move || {
            let out = package::applied(&doc)?;
            package::build_saved(&dir, &doc, &out, None)?;
            Ok(out.to_string_lossy().into_owned())
        };
        self.work(
            job,
            Box::new(move |mut editor, path| {
                editor.as_mut().rescan();
                let qt = editor.qt_thread();
                bus::spawn(async move {
                    let done = install(path, output).await;
                    qt.queue(move |mut editor| match done {
                        Ok(uid) => editor.as_mut().finished(QString::from(&uid)),
                        Err(e) => editor.fail(e),
                    })
                    .ok();
                });
            }),
        );
    }
}

async fn install(path: String, output: String) -> Result<String, String> {
    let mut events = bus::events();
    let message = |e: wprs_client::Error| e.message;
    let reply = bus::call("Library1", "Import", vec![json!([path]), json!(false)]).await.map_err(message)?;
    let job =
        serde_json::from_str::<Value>(&reply).ok().and_then(|r| r[0].as_u64()).ok_or("no import job")?;
    loop {
        match events.recv().await {
            Some(Event::Finished(id, state, _, error)) if id == job && state != "done" => return Err(error),
            Some(Event::Finished(id, ..)) if id == job => break,
            Some(_) => {}
            None => return Err("the daemon stopped answering".into()),
        }
    }
    let reply = bus::call("Daemon1", "GetJob", vec![json!(job)]).await.map_err(message)?;
    let report = serde_json::from_str::<Value>(&reply).unwrap_or_default();
    let output_report = &report["job"]["output"];
    let uid = output_report["imported"][0]["uid"].as_str().map(String::from);
    let uid =
        uid.ok_or_else(|| output_report["failed"][0]["error"].as_str().unwrap_or("not imported").to_owned())?;
    bus::call("Library1", "Apply", vec![json!(uid), json!(output), json!(""), json!("")])
        .await
        .map_err(message)?;
    Ok(uid)
}
