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
        fn call(self: Pin<&mut Backend>, tag: &QString, iface: &QString, member: &QString, args: &QString);
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
        fn tr(self: &Backend, text: &QString) -> QString;
        #[qinvokable]
        fn url(self: &Backend, path: &QString) -> QString;
        #[qinvokable]
        fn path(self: &Backend, url: &QString) -> QString;
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
        fn activated(self: Pin<&mut Backend>);
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
use std::pin::Pin;
use wprs_client::Error;
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
    pub fn call(self: Pin<&mut Self>, tag: &QString, iface: &QString, member: &QString, args: &QString) {
        let (iface, member, args) = (iface.to_string(), member.to_string(), args.to_string());
        self.run(tag, async move {
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

    pub fn url(&self, path: &QString) -> QString {
        QString::from(&bus::url(&path.to_string()))
    }

    pub fn path(&self, url: &QString) -> QString {
        QString::from(&app::path(&url.to_string()).unwrap_or_default())
    }

    pub fn run(
        self: Pin<&mut Self>,
        tag: &QString,
        task: impl Future<Output = Result<String, Error>> + Send + 'static,
    ) {
        let (qt, tag) = (self.qt_thread(), tag.to_string());
        bus::spawn(async move {
            let result = task.await;
            qt.queue(move |mut this| match result {
                Ok(json) => {
                    this.as_mut().set_last_error(QString::default());
                    this.replied(QString::from(&tag), QString::from(&json));
                }
                Err(e) => {
                    if this.checking {
                        eprintln!("call failed: {}", e.message);
                    }
                    this.as_mut().set_last_error(QString::from(&e.message));
                    this.failed(QString::from(&tag), QString::from(&e.name), QString::from(&e.message));
                }
            })
            .ok();
        });
    }
}
