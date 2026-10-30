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
        #[qproperty(QString, state)]
        #[qproperty(QString, schema)]
        #[qproperty(QString, jobs)]
        #[qproperty(QString, banner)]
        #[qproperty(QString, last_error)]
        #[qproperty(bool, checking)]
        type Backend = super::BackendRust;

        #[qinvokable]
        fn call(
            self: Pin<&mut Backend>,
            tag: &QString,
            iface: &QString,
            member: &QString,
            args: &QString,
            handled: bool,
        );
        #[qinvokable]
        fn add_provider(
            self: Pin<&mut Backend>,
            tag: &QString,
            id: &QString,
            kind: &QString,
            options: &QString,
            secret: &QString,
        );
        #[qinvokable]
        fn set_secret(self: Pin<&mut Backend>, tag: &QString, id: &QString, secret: &QString);
        #[qinvokable]
        fn passphrase(self: Pin<&mut Backend>, tag: &QString, member: &QString, passphrase: &QString);
        #[qinvokable]
        fn scene_secret(
            self: Pin<&mut Backend>,
            tag: &QString,
            output: &QString,
            key: &QString,
            secret: &QString,
        );
        #[qinvokable]
        fn write_text(self: &Backend, url: &QString, text: &QString) -> bool;
        #[qinvokable]
        fn read_text(self: &Backend, url: &QString) -> QString;
        #[qinvokable]
        fn tr(self: &Backend, text: &QString) -> QString;
        #[qinvokable]
        fn url(self: &Backend, path: &QString) -> QString;
        #[qinvokable]
        fn requested(self: &Backend) -> QString;
        #[qinvokable]
        fn path(self: &Backend, url: &QString) -> QString;
        #[qinvokable]
        fn markup(self: &Backend, text: &QString) -> QString;
        #[qinvokable]
        fn plain(self: &Backend, text: &QString) -> QString;
        #[qsignal]
        fn replied(self: Pin<&mut Backend>, tag: QString, json: QString);
        #[qsignal]
        fn failed(self: Pin<&mut Backend>, tag: QString, name: QString, message: QString);
        #[qsignal]
        fn changed(self: Pin<&mut Backend>, topic: QString, ids: QString, serial: u64);
        #[qsignal]
        fn job_progress(self: Pin<&mut Backend>, job: u64, done: u64, total: u64);
        #[qsignal]
        fn job_finished(self: Pin<&mut Backend>, job: u64, state: QString, name: QString, message: QString);
        #[qsignal]
        fn thumbnail_ready(self: Pin<&mut Backend>, uid: QString, size: u32, path: QString);
        #[qsignal]
        fn preview_ready(self: Pin<&mut Backend>, spec: QString, path: QString);
        #[qsignal]
        fn activated(self: Pin<&mut Backend>, page: QString);
    }

    impl cxx_qt::Threading for Backend {}
    impl cxx_qt::Initialize for Backend {}
}

use crate::args::{carries_secret, invalid};
use crate::bus;
use crate::{app, tr};
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use qobject::Backend;
use serde_json::Value;
use std::io::Read;
use std::pin::Pin;
use wprs_core::model::Job;

#[derive(Default)]
pub struct BackendRust {
    state: QString,
    schema: QString,
    jobs: QString,
    banner: QString,
    last_error: QString,
    checking: bool,
    pub list: Vec<Job>,
    pub linked: bool,
}

impl cxx_qt::Initialize for Backend {
    fn initialize(mut self: Pin<&mut Self>) {
        self.as_mut().set_checking(bus::checking());
        bus::listen(self.qt_thread(), Backend::event);
    }
}

impl Backend {
    pub fn call(
        self: Pin<&mut Self>,
        tag: &QString,
        iface: &QString,
        member: &QString,
        args: &QString,
        handled: bool,
    ) {
        let (iface, member, args) = (iface.to_string(), member.to_string(), args.to_string());
        self.run(tag, handled, async move {
            let args: Vec<Value> = serde_json::from_str(&args).map_err(|e| invalid(e.to_string()))?;
            if carries_secret(&iface, &member, &args) {
                return Err(invalid("use setSecret, addProvider or passphrase"));
            }
            bus::call(&iface, &member, args).await
        });
    }

    pub fn tr(&self, text: &QString) -> QString {
        QString::from(&tr::tr(&text.to_string()))
    }

    pub fn requested(&self) -> QString {
        QString::from(&bus::requested().unwrap_or_default())
    }

    pub fn url(&self, path: &QString) -> QString {
        QString::from(&bus::url(&path.to_string()))
    }

    pub fn path(&self, url: &QString) -> QString {
        QString::from(&app::path(&url.to_string()).unwrap_or_default())
    }

    pub fn write_text(&self, url: &QString, text: &QString) -> bool {
        app::path(&url.to_string()).is_some_and(|path| std::fs::write(path, text.to_string()).is_ok())
    }

    pub fn read_text(&self, url: &QString) -> QString {
        let read = |path: String| {
            let mut text = String::new();
            std::fs::File::open(path).ok()?.take(256 << 10).read_to_string(&mut text).ok().map(|_| text)
        };
        QString::from(&app::path(&url.to_string()).and_then(read).unwrap_or_default())
    }

    pub fn markup(&self, text: &QString) -> QString {
        QString::from(&wprs_core::markup::html(&text.to_string()))
    }

    pub fn plain(&self, text: &QString) -> QString {
        QString::from(&wprs_core::markup::plain(&text.to_string()))
    }
}
