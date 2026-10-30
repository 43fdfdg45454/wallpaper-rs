use crate::backend::qobject::Backend;
use crate::bus::{self, Event};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::json;
use std::pin::Pin;
use wprs_client::Error;
use wprs_core::model::JobsReply;

pub(crate) const TOKEN: &str = "XDG_ACTIVATION_TOKEN";
pub(crate) const SILENT: &str = "The job failed without giving a reason";
const TOPICS: [&str; 9] =
    ["state", "library", "playlists", "profiles", "providers", "repos", "jobs", "inputs", "account"];

fn q(text: &str) -> QString {
    QString::from(text)
}

pub(crate) fn told(message: String, tr: impl Fn(&str) -> String) -> String {
    match wprs_core::dbus::error::silent(&message) {
        Some("") => tr(SILENT),
        Some(short) => format!("{} ({short})", tr(SILENT)),
        None => message,
    }
}

impl Backend {
    pub fn event(mut self: Pin<&mut Self>, event: Event) {
        match event {
            Event::Link(Ok(())) => {
                self.as_mut().set_banner(QString::default());
                for member in ["GetState", "GetSchema", "ListJobs"] {
                    self.as_mut().refresh(member);
                }
                if std::mem::replace(&mut self.as_mut().rust_mut().linked, true) {
                    for topic in TOPICS {
                        self.as_mut().changed(q(topic), q("[]"), 0);
                    }
                }
            }
            Event::Link(Err(message)) => {
                self.as_mut().rust_mut().linked = true;
                self.set_banner(q(&message));
            }
            Event::Changed(topic, ids, serial) => {
                match topic.as_str() {
                    "state" => self.as_mut().refresh("GetState"),
                    "jobs" => self.as_mut().refresh("ListJobs"),
                    _ => {}
                }
                self.changed(q(&topic), q(&json!(ids).to_string()), serial);
            }
            Event::Progress(job, done, total) => {
                if let Some(entry) = self.as_mut().rust_mut().list.iter_mut().find(|j| j.id == job) {
                    (entry.done, entry.total) = (done, total);
                }
                self.as_mut().show_jobs();
                self.job_progress(job, done, total);
            }
            Event::Finished(job, state, name, message) => {
                let message = if state == "failed" { told(message, crate::tr::tr) } else { message };
                if state == "failed" {
                    self.as_mut().set_last_error(q(&message));
                }
                self.job_finished(job, q(&state), q(&name), q(&message));
            }
            Event::Thumb(uid, size, path) => self.thumbnail_ready(q(&uid), size, q(&path)),
            Event::Preview(spec, path) => self.preview_ready(q(&spec), q(&path)),
            Event::Activate(page, token) => {
                if let Some(token) = token {
                    unsafe { std::env::set_var(TOKEN, token) };
                }
                self.activated(q(&page))
            }
            Event::Error(message) => self.set_last_error(q(&message)),
        }
    }

    fn refresh(self: Pin<&mut Self>, member: &'static str) {
        let qt = self.qt_thread();
        bus::spawn(async move {
            let reply = bus::call("Daemon1", member, Vec::new()).await;
            qt.queue(move |mut this| match (member, reply) {
                ("ListJobs", Ok(json)) => {
                    let mut jobs = wprs_client::parse::<JobsReply>(&json).map(|r| r.jobs).unwrap_or_default();
                    for error in jobs.iter_mut().filter_map(|j| j.error.as_mut()) {
                        error.message = told(std::mem::take(&mut error.message), crate::tr::tr);
                    }
                    this.as_mut().rust_mut().list = jobs;
                    this.show_jobs();
                }
                ("GetSchema", Ok(json)) => this.set_schema(q(&json)),
                ("GetState", Ok(json)) => this.set_state(q(&json)),
                ("GetState", Err(e)) => this.set_last_error(q(&e.message)),
                _ => {}
            })
            .ok();
        });
    }

    fn show_jobs(self: Pin<&mut Self>) {
        let jobs = serde_json::to_string(&self.list).unwrap_or_default();
        self.set_jobs(q(&jobs));
    }
}

impl Backend {
    pub fn run(
        self: Pin<&mut Self>,
        tag: &QString,
        handled: bool,
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
                Err(e) if handled => {
                    this.failed(QString::from(&tag), QString::from(&e.name), QString::from(&e.message))
                }
                Err(e) => {
                    if *this.checking() {
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
