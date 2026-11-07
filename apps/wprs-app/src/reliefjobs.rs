use crate::picture::{write, write_png};
use crate::relief::qobject::Relief;
use crate::reliefwork::{Target, build, outline, overlay, survey};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::Duration;
use wprs_core::Fill;
use wprs_depth::Places;

const SETTLE: Duration = Duration::from_millis(600);
const RETRY: Duration = Duration::from_millis(250);
const OVERLAY: usize = 0;
const OUTLINE: usize = 1;
const SURVEY: usize = 2;

pub(crate) fn url(path: &std::path::Path) -> QString {
    QString::from(&crate::bus::url(&path.to_string_lossy()))
}

pub(crate) fn forget(old: &QString) {
    crate::app::path(&old.to_string()).map(std::fs::remove_file);
}

impl Relief {
    pub fn open(mut self: Pin<&mut Self>, path: &QString) {
        let file = crate::app::path(&path.to_string()).map(PathBuf::from).unwrap_or_default();
        self.as_mut().rust_mut().file = file;
    }

    pub fn look(self: Pin<&mut Self>, edit: &QString, colour: bool) {
        self.quick(OVERLAY, format!("{colour}\n{edit}"));
    }

    pub fn trace(self: Pin<&mut Self>, edit: &QString, cuts: &QString) {
        self.quick(OUTLINE, format!("{cuts}\n{edit}"));
    }

    pub fn measure(self: Pin<&mut Self>, edit: &QString) {
        self.quick(SURVEY, edit.to_string());
    }

    fn quick(mut self: Pin<&mut Self>, kind: usize, args: String) {
        if std::mem::replace(&mut self.as_mut().rust_mut().working[kind], true) {
            self.as_mut().rust_mut().waiting[kind] = Some(args);
            return;
        }
        let (qt, file) = (self.qt_thread(), self.file.clone());
        std::thread::spawn(move || {
            let made = work(kind, &file, &args);
            qt.queue(move |mut relief| {
                relief.as_mut().rust_mut().working[kind] = false;
                match (kind, made) {
                    (SURVEY, Ok(Made::Text(text))) => relief.as_mut().set_survey(QString::from(&text)),
                    (_, Ok(Made::File(path))) => {
                        let old =
                            if kind == OVERLAY { relief.overlay.clone() } else { relief.outline.clone() };
                        match kind {
                            OVERLAY => relief.as_mut().set_overlay(url(&path)),
                            _ => relief.as_mut().set_outline(url(&path)),
                        }
                        forget(&old);
                    }
                    (_, Err(e)) => relief.as_mut().set_error(QString::from(&format!("{e:#}"))),
                    _ => {}
                }
                if let Some(next) = relief.as_mut().rust_mut().waiting[kind].take() {
                    relief.quick(kind, next);
                }
            })
            .ok();
        });
    }

    pub fn sync(mut self: Pin<&mut Self>, edit: &QString) {
        self.as_mut().rust_mut().stop.store(true, Relaxed);
        let (turn, edit) = (self.turn + 1, edit.to_string());
        self.as_mut().rust_mut().turn = turn;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            std::thread::sleep(SETTLE);
            qt.queue(move |relief| {
                if relief.turn == turn {
                    relief.rebuild(edit, None);
                }
            })
            .ok();
        });
    }

    pub(crate) fn rebuild(mut self: Pin<&mut Self>, edit: String, fill: Option<Fill>) {
        if self.busy {
            self.as_mut().cancel();
            return self.later(move |r| r.rebuild(edit, fill));
        }
        let stop = Arc::new(AtomicBool::new(false));
        self.as_mut().rust_mut().stop = stop.clone();
        self.as_mut().set_error(QString::default());
        self.as_mut().begin("building");
        let (qt, file) = (self.qt_thread(), self.file.clone());
        std::thread::spawn(move || {
            let report = qt.clone();
            let made = Target::new(&file, &edit).and_then(|target| {
                let mut progress = |done: u64| {
                    report.queue(move |mut r| r.as_mut().set_progress(done as f64 / 100.0)).ok();
                };
                build(&Places::default(), &target, fill, &stop, &mut progress)
            });
            let cancelled = stop.load(Relaxed);
            qt.queue(move |mut relief| {
                relief.as_mut().end();
                match made {
                    Err(_) if cancelled => {}
                    Err(e) => relief.as_mut().set_error(QString::from(&format!("{e:#}"))),
                    Ok(None) => {}
                    Ok(Some(_)) => {
                        let built = relief.built + 1;
                        relief.as_mut().set_built(built);
                    }
                }
            })
            .ok();
        });
    }

    pub(crate) fn later(self: Pin<&mut Self>, again: impl FnOnce(Pin<&mut Relief>) + Send + 'static) {
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            std::thread::sleep(RETRY);
            qt.queue(again).ok();
        });
    }

    pub(crate) fn begin(mut self: Pin<&mut Self>, stage: &str) {
        self.as_mut().set_stage(QString::from(stage));
        self.as_mut().set_progress(-1.0);
        self.as_mut().set_busy(true);
    }

    pub(crate) fn end(mut self: Pin<&mut Self>) {
        self.as_mut().set_stage(QString::default());
        self.as_mut().set_busy(false);
    }

    pub fn cancel(self: Pin<&mut Self>) {
        self.stop.store(true, Relaxed);
    }

    pub fn clear(mut self: Pin<&mut Self>) {
        let (old, older) = (self.shown.clone(), self.other.clone());
        self.as_mut().set_shown(QString::default());
        self.as_mut().set_other(QString::default());
        forget(&old);
        forget(&older);
    }
}

enum Made {
    File(PathBuf),
    Text(String),
}

fn work(kind: usize, file: &std::path::Path, args: &str) -> anyhow::Result<Made> {
    let (head, edit) = args.split_once('\n').unwrap_or(("", args));
    let (places, target) = (Places::default(), Target::new(file, if kind == SURVEY { args } else { edit })?);
    Ok(match kind {
        OVERLAY => Made::File(write(&overlay(&places, &target, head == "true")?)?),
        OUTLINE => {
            let cuts: Vec<f32> = serde_json::from_str(head)?;
            Made::File(write_png(&outline(&places, &target, &cuts)?)?)
        }
        _ => Made::Text(survey(&places, &target)?),
    })
}

impl Drop for crate::relief::ReliefRust {
    fn drop(&mut self) {
        [&self.overlay, &self.outline, &self.shown, &self.other].into_iter().for_each(forget);
        self.stop.store(true, Relaxed);
    }
}
