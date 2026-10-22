use crate::backend::qobject::Backend;
use crate::bus::{self, Event};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::json;
use std::pin::Pin;
use wprs_core::model::JobsReply;

const TOPICS: [&str; 9] =
    ["state", "library", "playlists", "profiles", "providers", "repos", "jobs", "inputs", "account"];

fn q(text: &str) -> QString {
    QString::from(text)
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
                if state == "failed" {
                    self.as_mut().set_last_error(q(&message));
                }
                self.job_finished(job, q(&state), q(&name), q(&message));
            }
            Event::Thumb(uid, size, path) => self.thumbnail_ready(q(&uid), size, q(&path)),
            Event::Preview(spec, path) => self.preview_ready(q(&spec), q(&path)),
            Event::Activate => self.activated(),
            Event::Error(message) => self.set_last_error(q(&message)),
        }
    }

    fn refresh(self: Pin<&mut Self>, member: &'static str) {
        let qt = self.qt_thread();
        bus::spawn(async move {
            let reply = bus::call("Daemon1", member, Vec::new()).await;
            qt.queue(move |mut this| match (member, reply) {
                ("ListJobs", Ok(json)) => {
                    let jobs = wprs_client::parse::<JobsReply>(&json).map(|r| r.jobs);
                    this.as_mut().rust_mut().list = jobs.unwrap_or_default();
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
