#![cfg_attr(not(target_os = "android"), allow(dead_code))]

mod account;
mod acting;
mod apply;
#[cfg(target_os = "android")]
mod bridge;
#[cfg(target_os = "android")]
mod calls;
mod choice;
#[cfg(target_os = "android")]
mod clips;
mod cycle;
mod depth;
#[cfg(target_os = "android")]
mod engine;
#[cfg(target_os = "android")]
mod exports;
mod framing;
#[cfg(target_os = "android")]
mod gpu;
mod hearing;
mod keys;
mod library;
mod link;
mod local;
mod media;
mod pack;
mod pairing;
mod power;
mod props;
#[cfg(target_os = "android")]
mod relief;
mod rotation;
mod saved;
mod settings;
mod stage;
mod state;
mod store;
#[cfg(test)]
mod tests;
mod tilt;
mod vault;

pub mod host {
    use crate::{Error, account, depth, library, settings, store};
    use serde_json::json;

    pub fn prepare(files: &str) -> Result<(), String> {
        store::prepare(files).map_err(|e| e.to_string())
    }

    pub fn account(request: &str) -> String {
        account::request(request)
    }

    pub fn library(request: &str) -> String {
        library::request(request, true)
    }

    pub fn depth(request: &str) -> String {
        depth::request(request)
    }

    pub fn import(path: &str) -> String {
        let stored = std::fs::File::open(path).map_err(Error::from);
        let stored = stored.and_then(|mut file| store::store(&mut file, path));
        stored.map_or_else(|e| e.reply(), |(uid, _)| json!(uid)).to_string()
    }

    pub fn setting(key: &str, value: Option<&str>) -> String {
        settings::setting(key, value).unwrap_or_else(|e| e.to_string())
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[cfg(target_os = "android")]
    #[error(transparent)]
    Jni(#[from] jni::errors::Error),
    #[cfg(target_os = "android")]
    #[error(transparent)]
    Render(#[from] wprs_render::RenderError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Library(#[from] wprs_library::Error),
    #[error(transparent)]
    Config(#[from] wprs_core::ConfigError),
    #[error(transparent)]
    Image(#[from] image::ImageError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{0:#}")]
    Depth(#[from] anyhow::Error),
    #[error(transparent)]
    Sync(#[from] wprs_sync::Error),
    #[error("{0}")]
    Said(#[from] wprs_core::message::Message),
    #[error("not a supported image or video: {0}")]
    Unreadable(String),
    #[error(transparent)]
    Format(#[from] wprs_format::Error),
    #[error("{0}")]
    Setting(String),
    #[cfg(target_os = "android")]
    #[error("the surface has no native window")]
    Window,
    #[error("the app storage is not known yet")]
    Unprepared,
}

pub(crate) fn said(id: &'static str, text: &'static str, args: &[&dyn std::fmt::Display]) -> Error {
    Error::Said(wprs_core::message::said(id, text, args))
}

impl Error {
    pub(crate) fn told(&self) -> String {
        self.told_in(&library::languages())
    }

    pub(crate) fn reply(&self) -> serde_json::Value {
        let (id, args) = match self {
            Error::Said(m) => (m.id, m.args.clone()),
            Error::Format(e) => {
                let message = e.message();
                (message.id, message.args)
            }
            Error::Unreadable(name) => ("android.unreadable", vec![name.clone()]),
            Error::Library(wprs_library::Error::NotFound(what, key)) => {
                ("android.not_found", vec![(*what).to_owned(), key.clone()])
            }
            Error::Library(wprs_library::Error::Exists(what, key)) => {
                ("android.exists", vec![(*what).to_owned(), key.clone()])
            }
            Error::Io(e) => ("android.io", vec![e.to_string()]),
            _ => ("", Vec::new()),
        };
        serde_json::json!({ "error": self.told(), "id": id, "args": args })
    }

    pub(crate) fn told_in(&self, languages: &[String]) -> String {
        match self {
            Error::Format(e) => e.message().told(&wprs_core::message::tr_in(languages)),
            e => e.to_string(),
        }
    }
}

#[cfg(target_os = "android")]
#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(
        priority: std::ffi::c_int,
        tag: *const std::ffi::c_char,
        text: *const std::ffi::c_char,
    ) -> std::ffi::c_int;
}

#[cfg(target_os = "android")]
struct Logcat;

#[cfg(target_os = "android")]
impl log::Log for Logcat {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata())
            && let Ok(text) = std::ffi::CString::new(record.args().to_string())
        {
            unsafe {
                __android_log_write(7 - record.level() as std::ffi::c_int, c"wprs".as_ptr(), text.as_ptr())
            };
        }
    }

    fn flush(&self) {}
}
