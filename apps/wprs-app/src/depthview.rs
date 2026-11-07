use crate::depth::DepthRust;
use crate::depth::qobject::DepthMaps;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use std::pin::Pin;
use wprs_core::edit::Depth;
use wprs_depth::Places;

impl DepthMaps {
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
}

fn bus_url(path: &std::path::Path) -> String {
    crate::bus::url(&path.to_string_lossy())
}

impl Drop for DepthRust {
    fn drop(&mut self) {
        crate::app::path(&self.look.to_string()).map(std::fs::remove_file);
    }
}
