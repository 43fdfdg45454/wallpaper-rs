//! Command line control for the wallpaper-rs daemon.

use anyhow::Context;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[zbus::proxy(
    interface = "io.github.wallpaper_rs.Daemon1",
    default_service = "io.github.wallpaper_rs.Daemon",
    default_path = "/io/github/wallpaper_rs/Daemon"
)]
trait Daemon {
    fn request_stream(&self, output: &str, width: u32, height: u32) -> zbus::Result<(u32, u64)>;
    fn set_wallpaper(&self, path: &str, output: &str, fit: &str) -> zbus::Result<()>;
    fn reload(&self) -> zbus::Result<()>;
    fn get_state(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn version(&self) -> zbus::Result<String>;
}

#[derive(Parser)]
#[command(name = "wprs-ctl", version, about = "Control the wallpaper-rs daemon")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Set the wallpaper image.
    Set {
        /// Image file.
        path: PathBuf,
        /// Output connector name (e.g. DP-1). Omit to apply to every output.
        #[arg(short, long, default_value = "")]
        output: String,
        /// Placement: fill, fit, stretch, center or tile.
        #[arg(short, long)]
        fit: Option<wprs_core::Fit>,
    },
    /// Re-read the configuration file.
    Reload,
    /// Print the daemon state as JSON.
    State,
    /// Create a stream for an output and print its node id and serial (diagnostics).
    Stream { output: String, width: u32, height: u32 },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let connection = zbus::Connection::session()
        .await
        .context("cannot connect to the session bus")?;
    let daemon = DaemonProxy::new(&connection).await?;
    let not_running = || format!("is wprsd running? ({} not reachable)", wprs_core::dbus::BUS_NAME);

    match cli.command {
        Command::Set { path, output, fit } => {
            let path =
                std::path::absolute(&path).with_context(|| format!("invalid path {}", path.display()))?;
            let path = path.to_str().context("the path is not valid UTF-8")?;
            let fit = fit.map(|f| f.as_str()).unwrap_or("");
            daemon
                .set_wallpaper(path, &output, fit)
                .await
                .with_context(not_running)?;
        }
        Command::Reload => daemon.reload().await.with_context(not_running)?,
        Command::State => println!("{}", daemon.get_state().await.with_context(not_running)?),
        Command::Stream {
            output,
            width,
            height,
        } => {
            let (node, serial) = daemon
                .request_stream(&output, width, height)
                .await
                .with_context(not_running)?;
            println!("node {node} serial {serial}");
        }
    }
    Ok(())
}
