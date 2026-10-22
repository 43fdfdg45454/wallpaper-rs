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
        fn install(self: Pin<&mut DepthMaps>, file: &QString);
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

    pub fn install(self: Pin<&mut Self>, file: &QString) {
        let given = crate::app::path(&file.to_string()).map(PathBuf::from).unwrap_or_default();
        self.start(Work::Install(given));
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
        std::thread::spawn(move || {
            let report = qt.clone();
            let target = Target { image, sha, depth };
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
