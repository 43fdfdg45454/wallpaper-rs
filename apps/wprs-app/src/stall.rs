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
        fn sample(self: &Stall) -> QString;
        #[qinvokable]
        fn add(self: Pin<&mut Stall>, folder: &QString, files: &QString, unlisted: bool);
        #[qinvokable]
        fn publish(self: Pin<&mut Stall>, folder: &QString, key: &QString, password: &QString, days: u32);
        #[qinvokable]
        fn list(self: Pin<&mut Stall>, folder: &QString, id: &QString, version: &QString, listed: bool);
        #[qinvokable]
        fn feature(self: Pin<&mut Stall>, folder: &QString, id: &QString, note: &QString, on: bool);
        #[qinvokable]
        fn revoke(
            self: Pin<&mut Stall>,
            folder: &QString,
            id: &QString,
            version: &QString,
            reason: &QString,
            message: &QString,
        );
        #[qinvokable]
        fn unrevoke(self: Pin<&mut Stall>, folder: &QString, id: &QString, version: &QString);
        #[qinvokable]
        fn revoke_key(self: Pin<&mut Stall>, folder: &QString, fingerprint: &QString, reason: &QString);
        #[qinvokable]
        fn create(self: Pin<&mut Stall>, folder: &QString, charter: &QString);
        #[qinvokable]
        fn rotate(self: Pin<&mut Stall>, folder: &QString, charter: &QString);
        #[qinvokable]
        fn sign_root(self: Pin<&mut Stall>, folder: &QString, key: &QString, password: &QString);
        #[qsignal]
        fn answered(self: Pin<&mut Stall>, answer: QString);
    }

    impl cxx_qt::Threading for Stall {}
}

use crate::bus;
use crate::scenefile::Shown;
use crate::stallwork::{added, changed, chartered, published, rooted};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use qobject::Stall;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Mutex, PoisonError};
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

    pub fn sample(&self) -> QString {
        answer(crate::stallcheck::sample())
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

    pub fn revoke(
        self: Pin<&mut Self>,
        folder: &QString,
        id: &QString,
        version: &QString,
        reason: &QString,
        message: &QString,
    ) {
        let (dir, id, version) = (PathBuf::from(folder.to_string()), id.to_string(), text(version));
        let (reason, message) = (reason.to_string(), message.to_string());
        self.behind(move || changed(stall::revoked(&dir, &id, version.as_deref(), &reason, &message)));
    }

    pub fn unrevoke(self: Pin<&mut Self>, folder: &QString, id: &QString, version: &QString) {
        let (dir, id) = (PathBuf::from(folder.to_string()), id.to_string());
        let version = text(version).unwrap_or_else(|| "*".into());
        self.behind(move || changed(stall::unrevoked(&dir, &id, &version)));
    }

    pub fn revoke_key(self: Pin<&mut Self>, folder: &QString, fingerprint: &QString, reason: &QString) {
        let (dir, key, reason) =
            (PathBuf::from(folder.to_string()), fingerprint.to_string(), reason.to_string());
        self.behind(move || changed(stall::key_revoked(&dir, &key, &reason)));
    }

    pub fn create(self: Pin<&mut Self>, folder: &QString, charter: &QString) {
        let (dir, charter) = (PathBuf::from(folder.to_string()), charter.to_string());
        self.behind(move || chartered(&dir, &charter, true));
    }

    pub fn rotate(self: Pin<&mut Self>, folder: &QString, charter: &QString) {
        let (dir, charter) = (PathBuf::from(folder.to_string()), charter.to_string());
        self.behind(move || chartered(&dir, &charter, false));
    }

    pub fn sign_root(self: Pin<&mut Self>, folder: &QString, key: &QString, password: &QString) {
        let (dir, key, password) =
            (PathBuf::from(folder.to_string()), PathBuf::from(key.to_string()), text(password));
        self.behind(move || rooted(&dir, &key, password));
    }
}
