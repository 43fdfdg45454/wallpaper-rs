use crate::daemon::Daemon;
use crate::error::{Result, failed, invalid, not_found};
use std::sync::Weak;

pub struct Rgb(pub Weak<Daemon>);

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(work).await.map_err(|e| failed(e.to_string()))
}

#[zbus::interface(name = "io.github.wallpaper_rs.Rgb1")]
impl Rgb {
    #[zbus(out_args("json"))]
    async fn get_status(&self) -> Result<String> {
        let d = self.0.upgrade().ok_or_else(|| failed("the daemon is stopping"))?;
        let rgb = d.state().config.rgb.clone();
        Ok(crate::rgb::status(&rgb).to_string())
    }

    async fn connect(&self) -> Result<()> {
        blocking(crate::rgb::connect).await?.map_err(failed)
    }

    async fn test(&self, device: i32) -> Result<()> {
        let device = match device {
            -1 => None,
            d => Some(u32::try_from(d).map_err(|_| invalid(format!("{d} is not a device index or -1")))?),
        };
        blocking(move || crate::rgb::test(device)).await?.map_err(failed)
    }

    #[zbus(out_args("width", "height", "rgb"))]
    async fn get_strip(&self, output: String) -> Result<(u32, u32, Vec<u8>)> {
        let strip = crate::rgb::strip(&output);
        let strip = strip.ok_or_else(|| not_found(format!("{output} sends no colors to RGB lighting")))?;
        Ok((strip.width, strip.height, strip.rgb))
    }
}
