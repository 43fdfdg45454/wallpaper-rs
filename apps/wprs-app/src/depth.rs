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
        #[qproperty(QString, assets)]
        #[qproperty(QString, map)]
        #[qproperty(QString, stage)]
        #[qproperty(QString, error)]
        #[qproperty(f64, progress)]
        #[qproperty(bool, busy)]
        #[qproperty(bool, ready)]
        #[qproperty(bool, allowed)]
        #[qproperty(QString, device)]
        #[qproperty(QString, look)]
        type DepthMaps = super::DepthRust;

        #[qinvokable]
        fn open(self: Pin<&mut DepthMaps>, path: &QString);
        #[qinvokable]
        fn download(self: Pin<&mut DepthMaps>);
        #[qinvokable]
        fn generate(self: Pin<&mut DepthMaps>, input: u32);
        #[qinvokable]
        fn cancel(self: Pin<&mut DepthMaps>);
        #[qinvokable]
        fn adopt(self: Pin<&mut DepthMaps>, file: &QString);
        #[qinvokable]
        fn keep(self: Pin<&mut DepthMaps>);
        #[qinvokable]
        fn forget(self: Pin<&mut DepthMaps>);
        #[qinvokable]
        fn install(self: Pin<&mut DepthMaps>, file: &QString);
        #[qinvokable]
        fn view(self: Pin<&mut DepthMaps>, depth: &QString, colour: bool);
    }

    impl cxx_qt::Threading for DepthMaps {}
}

use crate::depthwork::{Target, Work, run};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use qobject::DepthMaps;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use wprs_core::edit::Depth;
use wprs_depth::Places;

#[derive(Default)]
pub struct DepthRust {
    pub assets: QString,
    pub map: QString,
    pub stage: QString,
    pub error: QString,
    pub progress: f64,
    pub busy: bool,
    pub ready: bool,
    pub allowed: bool,
    pub device: QString,
    pub look: QString,
    pub viewing: bool,
    pub again: Option<(String, bool)>,
    pub file: PathBuf,
    pub sha: String,
    pub input: u32,
    pub stop: Arc<AtomicBool>,
}

impl DepthMaps {
    pub fn open(mut self: Pin<&mut Self>, path: &QString) {
        let file = crate::app::path(&path.to_string()).map(PathBuf::from).unwrap_or_default();
        (self.as_mut().rust_mut().file, self.as_mut().rust_mut().sha) = (file, String::new());
        self.start(Work::Open);
    }

    pub fn download(self: Pin<&mut Self>) {
        self.start(Work::Download);
    }

    pub fn generate(mut self: Pin<&mut Self>, input: u32) {
        self.as_mut().rust_mut().input = input;
        self.start(Work::Generate);
    }

    pub fn cancel(self: Pin<&mut Self>) {
        self.stop.store(true, Ordering::Relaxed);
    }

    pub fn adopt(self: Pin<&mut Self>, file: &QString) {
        let given = crate::app::path(&file.to_string()).map(PathBuf::from).unwrap_or_default();
        self.start(Work::Adopt(given));
    }

    pub fn keep(self: Pin<&mut Self>) {
        self.start(Work::Keep);
    }

    pub fn forget(self: Pin<&mut Self>) {
        self.start(Work::Remove);
    }

    pub fn install(self: Pin<&mut Self>, file: &QString) {
        let given = crate::app::path(&file.to_string()).map(PathBuf::from).unwrap_or_default();
        self.start(Work::Install(given));
    }

    pub fn view(mut self: Pin<&mut Self>, depth: &QString, colour: bool) {
        if std::mem::replace(&mut self.as_mut().rust_mut().viewing, true) {
            self.as_mut().rust_mut().again = Some((depth.to_string(), colour));
            return;
        }
        let depth: Depth = serde_json::from_str(&depth.to_string()).unwrap_or_default();
        let (qt, image, sha) = (self.qt_thread(), self.file.clone(), self.sha.clone());
        std::thread::spawn(move || {
            let shown = crate::depthwork::look(&Places::default(), (&image, &sha), &depth, colour);
            qt.queue(move |mut maps| {
                let old = crate::app::path(&maps.look.to_string());
                maps.as_mut().set_look(QString::from(&shown.map(|p| bus_url(&p)).unwrap_or_default()));
                old.map(std::fs::remove_file);
                maps.as_mut().rust_mut().viewing = false;
                if let Some((depth, colour)) = maps.as_mut().rust_mut().again.take() {
                    maps.view(&QString::from(&depth), colour);
                }
            })
            .ok();
        });
    }

    fn start(mut self: Pin<&mut Self>, work: Work) {
        if self.busy {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        self.as_mut().rust_mut().stop = stop.clone();
        self.as_mut().set_error(QString::default());
        self.as_mut().set_progress(-1.0);
        self.as_mut().set_busy(true);
        let (qt, image, sha) = (self.qt_thread(), self.file.clone(), self.sha.clone());
        let depth = Depth {
            input: Some(self.input).filter(|i| *i > 0).unwrap_or(Depth::default().input),
            ..Depth::default()
        };
        let device = self.device.to_string().parse().unwrap_or_default();
        std::thread::spawn(move || {
            let report = qt.clone();
            let target = Target { image, sha, depth, device };
            let (shown, failed) = run(&Places::default(), target, work, &stop, &mut |stage, progress| {
                let stage = QString::from(stage);
                report
                    .queue(move |mut maps| {
                        maps.as_mut().set_stage(stage);
                        maps.as_mut().set_progress(progress);
                    })
                    .ok();
            });
            qt.queue(move |mut maps| {
                maps.as_mut().rust_mut().sha = shown.sha;
                maps.as_mut().set_assets(QString::from(&shown.assets));
                maps.as_mut().set_ready(shown.ready);
                maps.as_mut().set_allowed(shown.allowed);
                maps.as_mut().set_map(QString::default());
                maps.as_mut().set_map(QString::from(&shown.map));
                maps.as_mut().set_error(QString::from(failed.as_deref().unwrap_or_default()));
                maps.as_mut().set_stage(QString::default());
                maps.as_mut().set_busy(false);
            })
            .ok();
        });
    }
}

fn bus_url(path: &std::path::Path) -> String {
    crate::bus::url(&path.to_string_lossy())
}

impl Drop for DepthRust {
    fn drop(&mut self) {
        crate::app::path(&self.look.to_string()).map(std::fs::remove_file);
    }
}
