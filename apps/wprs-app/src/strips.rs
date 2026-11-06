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
        #[qproperty(QString, frames)]
        type Strips = super::StripsRust;

        #[qinvokable]
        fn ask(self: Pin<&mut Strips>, file: &QString, count: i32);
    }

    impl cxx_qt::Threading for Strips {}
}

use crate::bus;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use qobject::Strips;
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::pin::Pin;

#[derive(Default)]
pub struct StripsRust {
    pub frames: QString,
    pub asked: Vec<String>,
}

impl Strips {
    pub fn ask(mut self: Pin<&mut Self>, file: &QString, count: i32) {
        let file = file.to_string();
        if file.is_empty() || self.asked.contains(&file) {
            return;
        }
        self.as_mut().rust_mut().asked.push(file.clone());
        let (qt, count) = (self.qt_thread(), count.max(1) as usize);
        bus::spawn(async move {
            let path = PathBuf::from(&file);
            let found = tokio::task::spawn_blocking(move || wprs_editor::strips::strip(&path, count)).await;
            let found = found.unwrap_or(Value::Array(Vec::new()));
            qt.queue(move |mut strips| {
                let mut all: Map<String, Value> =
                    serde_json::from_str(&strips.frames.to_string()).unwrap_or_default();
                all.insert(file, found);
                strips.as_mut().set_frames(QString::from(&Value::Object(all).to_string()));
            })
            .ok();
        });
    }
}
