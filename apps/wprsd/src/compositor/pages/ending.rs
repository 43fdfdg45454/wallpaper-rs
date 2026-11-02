use super::super::state::State;
use super::super::{Command, Compositor, lock};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::Duration;

const CLEARING: Duration = Duration::from_secs(10);

type Ending = (Option<PathBuf>, JoinHandle<()>);

static ENDING: Mutex<Vec<Ending>> = Mutex::new(Vec::new());

fn quit(child: Child) {
    if !wprs_web::end(child, wprs_web::GRACE, wprs_web::DRAIN) {
        tracing::warn!("a web helper or its engine did not stop by itself and was stopped");
    }
}

fn background(job: impl FnOnce() + Send + 'static) -> io::Result<JoinHandle<()>> {
    std::thread::Builder::new().name("wprs-web-end".into()).spawn(job)
}

pub(super) fn later(child: Child, storage: Option<PathBuf>) {
    let held = Arc::new(Mutex::new(Some(child)));
    let theirs = held.clone();
    let mut ending = lock(&ENDING);
    ending.retain(|(_, handle)| !handle.is_finished());
    match background(move || lock(&theirs).take().into_iter().for_each(quit)) {
        Ok(handle) => ending.push((storage, handle)),
        Err(e) => {
            drop(ending);
            tracing::warn!("cannot stop a web helper in the background: {e}");
            lock(&held).take().into_iter().for_each(quit);
        }
    }
}

pub(super) fn busy(dir: &Path) -> bool {
    lock(&ENDING).iter().any(|(of, handle)| of.as_deref() == Some(dir) && !handle.is_finished())
}

pub(in crate::compositor) fn settle() {
    let all = std::mem::take(&mut *lock(&ENDING));
    all.into_iter().for_each(|(_, handle)| drop(handle.join()));
}

fn remove(dir: &Path) -> io::Result<()> {
    match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

pub(super) fn clear(dir: PathBuf, done: mpsc::Sender<io::Result<()>>) {
    let mut ending = lock(&ENDING);
    let (waits, rest): (Vec<Ending>, Vec<Ending>) =
        std::mem::take(&mut *ending).into_iter().partition(|(of, _)| of.as_ref() == Some(&dir));
    *ending = rest;
    let gone = dir.clone();
    let job = move || {
        waits.into_iter().for_each(|(_, handle)| drop(handle.join()));
        let _ = done.send(remove(&gone));
    };
    match background(job) {
        Ok(handle) => ending.push((Some(dir), handle)),
        Err(e) => tracing::warn!("cannot clear the data of a web page: {e}"),
    }
}

impl State {
    pub(in crate::compositor) fn clear_page(&mut self, dir: PathBuf, done: mpsc::Sender<io::Result<()>>) {
        self.pages.retain(|_, slot| slot.spec.storage.as_ref() != Some(&dir));
        clear(dir, done);
    }
}

impl Compositor {
    pub fn clear_page(&self, dir: PathBuf) -> io::Result<()> {
        let (done, answer) = mpsc::channel();
        if self.tx.send(Command::Clear(dir.clone(), done)).is_err() {
            return remove(&dir);
        }
        match answer.recv_timeout(CLEARING) {
            Ok(cleared) => cleared,
            Err(mpsc::RecvTimeoutError::Disconnected) => remove(&dir),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                Err(io::Error::new(io::ErrorKind::TimedOut, "the web page did not stop in time"))
            }
        }
    }
}
