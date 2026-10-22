use crate::bus::client;
use crate::daemon::Daemon;
use crate::streams::HELD;
use std::sync::Arc;
use zbus::fdo;
use zbus::message::Header;

pub struct Service(pub Arc<Daemon>);

#[zbus::interface(name = "io.github.wallpaper_rs.Daemon1")]
impl Service {
    async fn request_stream(
        &self,
        output: String,
        width: u32,
        height: u32,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] bus: &zbus::Connection,
    ) -> fdo::Result<(u32, u64)> {
        self.0.request(output, (width, height), client(&header), Some(bus)).await
    }

    async fn hold_stream(&self, output: String, width: u32, height: u32) -> fdo::Result<(u32, u64)> {
        self.0.request(output, (width, height), HELD.into(), None).await
    }

    async fn release_stream(&self, output: String, #[zbus(header)] header: Header<'_>) -> fdo::Result<()> {
        let client = client(&header);
        self.0.run(move |d| d.release(&output, &client)).await
    }

    async fn set_output_covered(
        &self,
        output: String,
        covered: bool,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<()> {
        let client = client(&header);
        self.0.run(move |d| d.set_covered(&output, &client, covered)).await
    }

    async fn set_wallpaper(&self, path: String, output: String, fit: String) -> fdo::Result<()> {
        self.0.run(move |d| d.change_wallpaper(&path, &output, &fit)).await
    }

    async fn set_background(&self, color: String, output: String) -> fdo::Result<()> {
        self.0.run(move |d| d.set_background(&color, &output)).await
    }

    async fn set_option(&self, key: String, value: String) -> fdo::Result<()> {
        self.0.run(move |d| d.set_option(&key, &value)).await
    }

    async fn reload(&self) -> fdo::Result<()> {
        self.0.run(Daemon::reload).await
    }

    async fn get_state(&self) -> String {
        self.0.state_json()
    }

    async fn set_paused(&self, paused: bool) -> fdo::Result<()> {
        self.0.run(move |d| d.set_paused(paused)).await
    }

    async fn set_volume(&self, volume: f64) -> fdo::Result<()> {
        self.0.run(move |d| d.set_audio(Some(volume), None)).await
    }

    async fn set_muted(&self, muted: bool) -> fdo::Result<()> {
        self.0.run(move |d| d.set_audio(None, Some(muted))).await
    }

    #[zbus(property(emits_changed_signal = "const"))]
    async fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_owned()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    async fn adapter(&self) -> String {
        self.0.adapter.clone()
    }

    #[zbus(property)]
    async fn paused(&self) -> bool {
        self.0.props.borrow().paused
    }

    #[zbus(property)]
    async fn volume(&self) -> f64 {
        self.0.props.borrow().volume
    }

    #[zbus(property)]
    async fn muted(&self) -> bool {
        self.0.props.borrow().muted
    }
}

#[test]
fn interface_name_matches_core_constant() {
    let literal = format!("#[zbus::interface(name = \"{}\")]", wprs_core::dbus::INTERFACE);
    assert!(include_str!("service.rs").contains(&literal));
}
