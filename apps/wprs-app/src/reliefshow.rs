use crate::picture::write_png;
use crate::relief::qobject::Relief;
use crate::reliefjobs::{forget, url};
use crate::reliefwork::{Target, build, layer};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use wprs_core::Fill;
use wprs_depth::Places;

impl Relief {
    pub fn view(self: Pin<&mut Self>, edit: &QString, layer: u32, band: bool) {
        self.show(edit.to_string(), vec![(layer as usize, band, None)]);
    }

    pub fn compare(self: Pin<&mut Self>, edit: &QString, layer: u32) {
        let layer = layer as usize;
        self.show(
            edit.to_string(),
            vec![(layer, false, Some(Fill::Lama)), (layer, false, Some(Fill::Simple))],
        );
    }

    fn show(mut self: Pin<&mut Self>, edit: String, views: Vec<(usize, bool, Option<Fill>)>) {
        if self.busy {
            self.as_mut().cancel();
            return self.later(move |r| r.show(edit, views));
        }
        let stop = Arc::new(AtomicBool::new(false));
        self.as_mut().rust_mut().stop = stop.clone();
        self.as_mut().set_error(QString::default());
        self.as_mut().begin("viewing");
        let (qt, file) = (self.qt_thread(), self.file.clone());
        std::thread::spawn(move || {
            let made = Target::new(&file, &edit).and_then(|target| {
                let places = Places::default();
                let mut images = Vec::new();
                for (k, (index, band, fill)) in views.iter().enumerate() {
                    let mut progress = |done: u64| {
                        let part = (k as f64 + done as f64 / 100.0) / views.len() as f64;
                        qt.queue(move |mut r| r.as_mut().set_progress(part)).ok();
                    };
                    build(&places, &target, *fill, &stop, &mut progress)?;
                    images.push(layer(&places, &target, (*index, *band), *fill)?);
                }
                images.iter().map(write_png).collect::<anyhow::Result<Vec<_>>>()
            });
            qt.queue(move |mut relief| {
                relief.as_mut().end();
                match made {
                    Err(e) => relief.as_mut().set_error(QString::from(&format!("{e:#}"))),
                    Ok(files) => {
                        let (old, older) = (relief.shown.clone(), relief.other.clone());
                        relief.as_mut().set_shown(files.first().map(|p| url(p)).unwrap_or_default());
                        relief.as_mut().set_other(files.get(1).map(|p| url(p)).unwrap_or_default());
                        forget(&old);
                        forget(&older);
                    }
                }
            })
            .ok();
        });
    }
}
