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
        #[qproperty(bool, busy)]
        type Stall = super::StallRust;

        #[qinvokable]
        fn open(self: &Stall, folder: &QString) -> QString;
        #[qinvokable]
        fn add(self: Pin<&mut Stall>, folder: &QString, files: &QString, unlisted: bool);
        #[qinvokable]
        fn publish(self: Pin<&mut Stall>, folder: &QString, key: &QString, password: &QString, days: u32);
        #[qinvokable]
        fn list(self: Pin<&mut Stall>, folder: &QString, id: &QString, version: &QString, listed: bool);
        #[qinvokable]
        fn feature(self: Pin<&mut Stall>, folder: &QString, id: &QString, note: &QString, on: bool);
        #[qsignal]
        fn answered(self: Pin<&mut Stall>, answer: QString);
    }

    impl cxx_qt::Threading for Stall {}
}

use crate::bus;
use crate::scenefile::Shown;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use qobject::Stall;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};
use wprs_format::stall;

static CATALOG: Mutex<()> = Mutex::new(());

#[derive(Default)]
pub struct StallRust {
    busy: bool,
    jobs: usize,
}

pub fn queued<T>(job: impl FnOnce() -> T) -> T {
    let _held = CATALOG.lock().unwrap_or_else(PoisonError::into_inner);
    job()
}

pub(crate) fn changed(state: wprs_format::Result<Value>) -> anyhow::Result<Value> {
    let mut state = state?;
    state["changed"] = json!(true);
    Ok(state)
}

fn now() -> anyhow::Result<i64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs().try_into()?)
}

pub fn added(dir: &Path, files: &[PathBuf], unlisted: bool) -> anyhow::Result<Value> {
    Ok(stall::added(dir, files, unlisted, now()?)?)
}

pub fn published(dir: &Path, key: &Path, password: Option<String>, days: u32) -> anyhow::Result<Value> {
    let signer = wprs_format::Signer::new(&std::fs::read_to_string(key)?, password)?;
    Ok(stall::published(dir, &signer, days, now()?)?)
}

fn answer(result: anyhow::Result<Value>) -> QString {
    let value = result.unwrap_or_else(|e| json!({ "error": e.shown() }));
    QString::from(&value.to_string())
}

fn text(q: &QString) -> Option<String> {
    Some(q.to_string()).filter(|t| !t.is_empty())
}

impl Stall {
    fn behind(mut self: Pin<&mut Self>, job: impl FnOnce() -> anyhow::Result<Value> + Send + 'static) {
        self.as_mut().rust_mut().jobs += 1;
        self.as_mut().set_busy(true);
        let qt = self.qt_thread();
        bus::spawn(async move {
            let result = tokio::task::spawn_blocking(move || queued(job));
            let reply = answer(result.await.map_err(anyhow::Error::from).and_then(|r| r));
            qt.queue(move |mut stall| {
                let jobs = stall.jobs.saturating_sub(1);
                stall.as_mut().rust_mut().jobs = jobs;
                stall.as_mut().set_busy(jobs > 0);
                stall.answered(reply);
            })
            .ok();
        });
    }

    pub fn open(&self, folder: &QString) -> QString {
        answer(stall::opened(Path::new(&folder.to_string())).map_err(Into::into))
    }

    pub fn add(self: Pin<&mut Self>, folder: &QString, files: &QString, unlisted: bool) {
        let (dir, files) = (PathBuf::from(folder.to_string()), files.to_string());
        self.behind(move || added(&dir, &serde_json::from_str::<Vec<PathBuf>>(&files)?, unlisted));
    }

    pub fn publish(self: Pin<&mut Self>, folder: &QString, key: &QString, password: &QString, days: u32) {
        let (dir, key, password) =
            (PathBuf::from(folder.to_string()), PathBuf::from(key.to_string()), text(password));
        self.behind(move || published(&dir, &key, password, days));
    }

    pub fn list(self: Pin<&mut Self>, folder: &QString, id: &QString, version: &QString, listed: bool) {
        let (dir, id, version) = (PathBuf::from(folder.to_string()), id.to_string(), text(version));
        self.behind(move || changed(stall::listed(&dir, &id, version.as_deref(), listed)));
    }

    pub fn feature(self: Pin<&mut Self>, folder: &QString, id: &QString, note: &QString, on: bool) {
        let (dir, id, note) = (PathBuf::from(folder.to_string()), id.to_string(), text(note));
        self.behind(move || changed(stall::featured(&dir, &id, note.as_deref(), on)));
    }
}
