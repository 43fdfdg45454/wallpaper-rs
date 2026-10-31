use crate::bus::client;
use crate::callers::caller;
use crate::daemon::Daemon;
use crate::error::Result;
use crate::items::reply;
use crate::streams::HELD;
use args::{absolute, local_path};
use std::sync::Weak;
use wprs_core::model::{JobReply, JobsReply, MODEL};
use zbus::fdo;
use zbus::message::Header;
use zbus::object_server::SignalEmitter as Emit;

mod args;

pub struct Service(pub Weak<Daemon>);

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
        self.d()?.render(move |d| d.release(&output, &client)).await
    }

    async fn set_output_covered(
        &self,
        output: String,
        covered: bool,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<()> {
        let client = client(&header);
        self.d()?.render(move |d| d.set_covered(&output, &client, covered)).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn set_output_info(
        &self,
        output: String,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        scale: f64,
        px_per_mm: f64,
        identity: String,
    ) -> Result<()> {
        let screen = wprs_core::Screen { x, y, width, height, scale, px_per_mm, identity };
        self.d()?.run(move |d| d.set_screen(&output, screen)).await
    }

    async fn show_calibration(&self, on: bool) -> Result<()> {
        self.d()?
            .run(move |d| {
                d.show_calibration(on);
                Ok::<_, fdo::Error>(())
            })
            .await
    }

    async fn preview_edit(&self, path: String, edit_json: String) -> Result<()> {
        let path = absolute(path)?;
        self.d()?.run(move |d| d.preview_edit(&path, &edit_json)).await
    }

    async fn preview_scene(
        &self,
        output: String,
        dir: String,
        document: String,
        time: f64,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] bus: &zbus::Connection,
    ) -> Result<()> {
        let dir = absolute(dir)?;
        let trusted = caller(bus, &header).await.trusted();
        self.d()?.run(move |d| d.preview_scene(&output, &dir, (&document, trusted), time)).await
    }

    async fn set_wallpaper(&self, path: String, output: String, fit: String) -> Result<()> {
        let path = local_path(&path)?;
        self.d()?.render(move |d| d.change_wallpaper(&path, &output, &fit)).await
    }

    async fn set_background(&self, color: String, output: String) -> Result<()> {
        self.d()?.render(move |d| d.set_background(&color, &output)).await
    }

    #[zbus(out_args("json"))]
    async fn get_scene_properties(&self, output: String) -> Result<String> {
        let d = self.d()?;
        let mut found = d.run(move |d| d.scene_properties(&output)).await?;
        d.fill_secrets(&mut found).await;
        reply(found)
    }

    async fn set_scene_property(&self, output: String, id: String, value_json: String) -> Result<()> {
        let (d, (o, i)) = (self.d()?, (output.clone(), id.clone()));
        match d.run(move |d| d.secret_key(&o, &i)).await? {
            Some(key) => d.store_secret(&key, &value_json).await,
            None => d.run(move |d| d.set_scene_property(&output, &id, &value_json)).await,
        }
    }

    #[zbus(out_args("preset"))]
    async fn save_scene_preset(&self, output: String, name: String) -> Result<String> {
        self.d()?.run(move |d| d.save_scene_preset(&output, &name)).await
    }

    async fn rename_scene_preset(&self, preset: String, name: String) -> Result<()> {
        self.d()?.run(move |d| d.rename_scene_preset(&preset, &name)).await
    }

    async fn delete_scene_preset(&self, preset: String) -> Result<()> {
        self.d()?.run(move |d| d.delete_scene_preset(&preset)).await
    }

    #[zbus(out_args("json"))]
    async fn export_scene_values(&self, output: String, preset: String) -> Result<String> {
        self.d()?.run(move |d| d.export_scene_values(&output, &preset)).await
    }

    #[zbus(out_args("warnings"))]
    async fn import_scene_values(&self, output: String, json: String, name: String) -> Result<Vec<String>> {
        self.d()?.run(move |d| d.import_scene_values(&output, &json, &name)).await
    }

    async fn press_scene_button(&self, output: String, id: String) -> Result<()> {
        self.d()?.render(move |d| d.press_scene_button(&output, &id)).await
    }

    async fn share_scene_values(&self, output: String, shared: bool) -> Result<()> {
        self.d()?.run(move |d| d.share_scene_values(&output, shared)).await
    }

    async fn apply_scene_preset(&self, output: String, preset: String) -> Result<()> {
        self.d()?.run(move |d| d.apply_scene_preset(&output, &preset)).await
    }

    async fn reset_scene_properties(&self, output: String) -> Result<()> {
        self.d()?.run(move |d| d.reset_scene_properties(&output)).await
    }

    async fn set_option(&self, key: String, value: String) -> Result<()> {
        self.d()?.render(move |d| d.set_option(&key, &value)).await
    }

    async fn reload(&self) -> Result<()> {
        self.d()?.render(|d| d.reload()).await
    }

    async fn retry_effects(&self) -> Result<()> {
        self.d()?.render(|d| d.retry_effects()).await
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
        self.d()?.render(move |d| d.set_paused(paused)).await
    }

    async fn set_volume(&self, volume: f64) -> Result<()> {
        self.d()?.render(move |d| d.set_audio(Some(volume), None)).await
    }

    async fn set_muted(&self, muted: bool) -> Result<()> {
        self.d()?.render(move |d| d.set_audio(None, Some(muted))).await
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
        wprs_core::VERSION.to_owned()
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
        e: &Emit<'_>,
        output: &str,
        node_id: u32,
        serial: u64,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn changed(e: &Emit<'_>, topic: &str, ids: Vec<String>, serial: u64)
    -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn job_progress(e: &Emit<'_>, job: u64, done: u64, total: u64) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn job_finished(
        e: &Emit<'_>,
        job: u64,
        state: &str,
        error_name: &str,
        message: &str,
    ) -> zbus::Result<()>;
}

#[cfg(test)]
mod tests;
