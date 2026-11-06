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
        #[qproperty(QString, trial)]
        #[qproperty(bool, busy)]
        #[qproperty(QString, flat)]
        type Packer = super::PackerRust;

        #[qinvokable]
        fn weigh(self: Pin<&mut Packer>, folder: &QString, args: &QString);

        #[qinvokable]
        fn flatten(self: Pin<&mut Packer>, file: &QString);
    }

    impl cxx_qt::Threading for Packer {}
}

use crate::bus;
use crate::scenefile::Shown;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use qobject::Packer;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::pin::Pin;
use wprs_editor::actions::AssetEdit;

#[derive(Default)]
pub struct PackerRust {
    pub trial: QString,
    pub busy: bool,
    pub flat: QString,
    pub serial: u64,
}

fn cache() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("wallpaper-rs").join("ktx2")
}

fn weighed(folder: PathBuf, args: Value) -> anyhow::Result<Value> {
    let wanted: AssetEdit = serde_json::from_value(json!({
        "asset": args["asset"], "ktx2": true, "codec": args["codec"], "quality": args["quality"]
    }))?;
    let doc = json!({ "scene": { "assets": { wanted.asset.clone(): args["entry"] } } });
    wprs_editor::packing::trial(&folder, &doc, &wanted, Some(&cache()))
}

impl Packer {
    pub fn weigh(mut self: Pin<&mut Self>, folder: &QString, args: &QString) {
        let folder = PathBuf::from(folder.to_string());
        let args = serde_json::from_str::<Value>(&args.to_string()).unwrap_or_default();
        let serial = {
            let mut this = self.as_mut().rust_mut();
            this.serial += 1;
            this.serial
        };
        self.as_mut().set_busy(true);
        let qt = self.qt_thread();
        bus::spawn(async move {
            let done = tokio::task::spawn_blocking(move || weighed(folder, args)).await;
            let value = match done.map_err(anyhow::Error::from).and_then(|r| r) {
                Ok(value) => value,
                Err(e) => json!({ "error": e.shown() }),
            };
            qt.queue(move |mut packer| {
                if packer.serial == serial {
                    packer.as_mut().set_busy(false);
                    packer.as_mut().set_trial(QString::from(&value.to_string()));
                }
            })
            .ok();
        });
    }

    pub fn flatten(mut self: Pin<&mut Self>, file: &QString) {
        let file = PathBuf::from(file.to_string());
        let serial = {
            let mut this = self.as_mut().rust_mut();
            this.serial += 1;
            this.serial
        };
        if file.as_os_str().is_empty() {
            return self.as_mut().set_flat(QString::default());
        }
        let qt = self.qt_thread();
        bus::spawn(async move {
            let shown = tokio::task::spawn_blocking(move || {
                wprs_editor::pictures::shown(file, &cache().join("shown"))
            });
            let shown = shown.await.ok().and_then(Result::ok).map(|p| p.to_string_lossy().into_owned());
            qt.queue(move |mut packer| {
                if packer.serial == serial {
                    packer.as_mut().set_flat(QString::from(shown.unwrap_or_default().as_str()));
                }
            })
            .ok();
        });
    }
}
