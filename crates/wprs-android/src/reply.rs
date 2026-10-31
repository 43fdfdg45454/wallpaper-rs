use crate::{Error, library};

impl Error {
    pub(crate) fn told(&self) -> String {
        self.told_in(&library::languages())
    }

    pub(crate) fn reply(&self) -> serde_json::Value {
        let text = |id, arg: String| (id, vec![arg]);
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
            Error::Library(e) => text("android.library", e.to_string()),
            Error::Io(e) => text("android.io", e.to_string()),
            Error::Sync(e) => synced(e),
            Error::Depth(e) => text("android.depth_failed", format!("{e:#}")),
            Error::Setting(e) => text("android.refused", e.clone()),
            Error::Config(e) => text("android.config", e.to_string()),
            Error::Image(e) => text("android.image", e.to_string()),
            Error::Json(e) => text("android.json", e.to_string()),
            Error::Unprepared => ("android.unprepared", Vec::new()),
            #[cfg(target_os = "android")]
            e @ (Error::Jni(_) | Error::Render(_) | Error::Window) => text("android.internal", e.to_string()),
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

fn synced(e: &wprs_sync::Error) -> (&'static str, Vec<String>) {
    use wprs_sync::Error::{Crypto, Invalid, Network, Server, Untrusted};
    match e {
        Untrusted => ("android.sync_untrusted", Vec::new()),
        Network(detail) => ("android.sync_network", vec![detail.clone()]),
        Server { detail, .. } => ("android.sync_server", vec![detail.clone()]),
        Crypto("the passphrase does not open the recovery escrow") => {
            ("android.wrong_passphrase", Vec::new())
        }
        Crypto(what) => ("android.sync_crypto", vec![(*what).to_owned()]),
        Invalid(what) => ("android.sync_invalid", vec![what.clone()]),
    }
}
