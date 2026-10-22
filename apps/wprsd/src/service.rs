use crate::bus::client;
use crate::daemon::Daemon;
use crate::error::{Result, failed};
use crate::items::reply;
use crate::streams::HELD;
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use wprs_core::dbus::uri_scheme;
use wprs_core::model::{JobReply, JobsReply, MODEL};
use zbus::fdo::{self, Error::InvalidArgs};
use zbus::message::Header;
use zbus::object_server::SignalEmitter;

pub struct Service(pub Weak<Daemon>);

fn local_path(path: &str) -> fdo::Result<PathBuf> {
    match uri_scheme(path) {
        None => Ok(path.into()),
        Some(scheme) if scheme.eq_ignore_ascii_case("file") => wprs_video::path_from_uri(path)
            .ok_or_else(|| InvalidArgs(format!("{path} is not a local file URI"))),
        Some(_) => Err(InvalidArgs(format!("{path} is not a local file"))),
    }
}

impl Service {
    fn d(&self) -> Result<Arc<Daemon>> {
        self.0.upgrade().ok_or_else(|| failed("the daemon is stopping"))
    }

    fn props(&self) -> crate::daemon::Props {
        self.0
            .upgrade()
            .map_or(crate::daemon::Props { paused: false, volume: 0.0, muted: true }, |d| *d.props.borrow())
    }
}

#[zbus::interface(name = "io.github.wallpaper_rs.Daemon1")]
impl Service {
    #[zbus(out_args("node_id", "serial"))]
    async fn request_stream(
        &self,
        output: String,
        width: u32,
        height: u32,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] bus: &zbus::Connection,
    ) -> Result<(u32, u64)> {
        self.d()?.request(output, (width, height), client(&header), Some(bus)).await
    }

    #[zbus(out_args("node_id", "serial"))]
    async fn hold_stream(&self, output: String, width: u32, height: u32) -> Result<(u32, u64)> {
        self.d()?.request(output, (width, height), HELD.into(), None).await
    }

    async fn release_stream(&self, output: String, #[zbus(header)] header: Header<'_>) -> Result<()> {
        let client = client(&header);
        self.d()?.run(move |d| d.release(&output, &client)).await
    }

    async fn set_output_covered(
        &self,
        output: String,
        covered: bool,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<()> {
        let client = client(&header);
        self.d()?.run(move |d| d.set_covered(&output, &client, covered)).await
    }

    async fn set_wallpaper(&self, path: String, output: String, fit: String) -> Result<()> {
        let path = local_path(&path)?;
        self.d()?.run(move |d| d.change_wallpaper(&path, &output, &fit)).await
    }

    async fn set_background(&self, color: String, output: String) -> Result<()> {
        self.d()?.run(move |d| d.set_background(&color, &output)).await
    }

    async fn set_option(&self, key: String, value: String) -> Result<()> {
        self.d()?.run(move |d| d.set_option(&key, &value)).await
    }

    async fn reload(&self) -> Result<()> {
        self.d()?.run(|d| d.reload()).await
    }

    #[zbus(out_args("json"))]
    async fn get_state(&self) -> Result<String> {
        Ok(self.d()?.state_json())
    }

    #[zbus(out_args("json"))]
    async fn get_schema(&self) -> Result<String> {
        reply(crate::schema::schema(self.d()?.changes.serial()))
    }

    async fn set_paused(&self, paused: bool) -> Result<()> {
        self.d()?.run(move |d| d.set_paused(paused)).await
    }

    async fn set_volume(&self, volume: f64) -> Result<()> {
        self.d()?.run(move |d| d.set_audio(Some(volume), None)).await
    }

    async fn set_muted(&self, muted: bool) -> Result<()> {
        self.d()?.run(move |d| d.set_audio(None, Some(muted))).await
    }

    #[zbus(out_args("serial", "topics"))]
    async fn wait_changed(&self, serial: u64, timeout_ms: u32) -> Result<(u64, Vec<String>)> {
        Ok(self.d()?.changes.wait(serial, timeout_ms).await)
    }

    #[zbus(out_args("json"))]
    async fn list_jobs(&self) -> Result<String> {
        let d = self.d()?;
        let serial = d.changes.serial();
        reply(JobsReply { model: MODEL, serial, jobs: d.jobs.list() })
    }

    #[zbus(out_args("json"))]
    async fn get_job(&self, job: u64) -> Result<String> {
        let d = self.d()?;
        let serial = d.changes.serial();
        reply(JobReply { model: MODEL, serial, job: d.jobs.get(job)? })
    }

    async fn cancel_job(&self, job: u64) -> Result<()> {
        self.d()?.cancel_job(job)
    }

    #[zbus(property(emits_changed_signal = "const"))]
    async fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_owned()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    async fn adapter(&self) -> String {
        self.0.upgrade().map(|d| d.adapter.clone()).unwrap_or_default()
    }

    #[zbus(property)]
    async fn paused(&self) -> bool {
        self.props().paused
    }

    #[zbus(property)]
    async fn volume(&self) -> f64 {
        self.props().volume
    }

    #[zbus(property)]
    async fn muted(&self) -> bool {
        self.props().muted
    }

    #[zbus(property(emits_changed_signal = "false"))]
    async fn serial(&self) -> u64 {
        self.0.upgrade().map_or(0, |d| d.changes.serial())
    }

    #[zbus(property(emits_changed_signal = "const"))]
    async fn model(&self) -> u32 {
        MODEL
    }

    #[zbus(signal)]
    pub(crate) async fn stream_changed(
        emitter: &SignalEmitter<'_>,
        output: &str,
        node_id: u32,
        serial: u64,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn changed(
        emitter: &SignalEmitter<'_>,
        topic: &str,
        ids: Vec<String>,
        serial: u64,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn job_progress(
        emitter: &SignalEmitter<'_>,
        job: u64,
        done: u64,
        total: u64,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn job_finished(
        emitter: &SignalEmitter<'_>,
        job: u64,
        state: &str,
        error_name: &str,
        message: &str,
    ) -> zbus::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_name_matches_core_constant() {
        let literal = format!("#[zbus::interface(name = \"{}\")]", wprs_core::dbus::INTERFACE);
        assert!(include_str!("service.rs").contains(&literal));
    }

    #[test]
    fn only_local_files_are_wallpapers() {
        assert_eq!(local_path("file:///tmp/a%20b.png").ok(), Some("/tmp/a b.png".into()));
        assert_eq!(local_path("/tmp/a:b.png").ok(), Some("/tmp/a:b.png".into()));
        for uri in ["http://127.0.0.1:9/x.webm", "data:image/png;base64,AA", "file://elsewhere/x.png"] {
            assert!(matches!(local_path(uri), Err(InvalidArgs(_))), "{uri}");
        }
    }
}
