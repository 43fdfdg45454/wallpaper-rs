mod account;
mod activate;
mod apply;
mod bus;
mod calibration;
mod callers;
mod catalogue;
mod changes;
mod choice;
mod clock;
mod compositor;
mod confirm;
mod controls;
mod daemon;
mod decode;
mod drafts;
mod editpreview;
mod edits;
mod error;
mod examples;
mod flatpak;
mod folders;
mod import;
mod inputs;
mod inputs1;
mod items;
mod jobs;
mod keyring;
mod kinds;
mod library1;
mod look;
mod orphans;
mod packages;
mod picks;
mod pixels;
mod playback;
mod playlists;
mod playlists1;
mod pool;
mod present;
mod presets;
mod previews;
mod profiles;
mod profiles1;
mod provider_edits;
mod providers;
mod providers1;
mod relief;
mod remote;
mod repos;
mod repos1;
mod reveals;
mod rotation;
mod scan;
mod scenepresets;
mod sceneprops;
mod scenes;
mod scenevalues;
mod schema;
mod secrets;
mod serve;
mod service;
#[cfg(test)]
mod spec;
mod store;
mod streams;
mod surfaces;
mod sync;
mod sync1;
mod thumbs;
mod tidy;
mod timing;
mod wallpaper;
mod watch;
mod words;
mod worker;
mod zone;

use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    version = wprs_core::VERSION,
    about = "Renders wallpapers and shows them through PipeWire streams, layer-shell surfaces or X11 windows"
)]
pub(crate) struct Args {
    #[arg(long, help = "Configuration file to use instead of the default one")]
    pub(crate) config: Option<PathBuf>,
    #[arg(long, hide = true)]
    worker: bool,
    #[arg(long, hide = true)]
    script_host: bool,
    #[arg(long, value_enum, default_value = "auto", help = "How wallpapers reach the desktop")]
    pub(crate) backend: present::Backend,
}

fn main() -> anyhow::Result<()> {
    wprs_scene::serve_check();
    wprs_scene::check_out_of_process(true);
    let args = Args::parse();
    if args.worker {
        return worker::run();
    }
    if args.script_host {
        return Ok(wprs_script::serve()?);
    }
    skip_vulkan_device_selection_layer();
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let quiet =
        ["zbus", "hyper", "hyper_util", "h2", "reqwest"].map(|target| format!("{target}=warn").parse());
    let filter = quiet.into_iter().flatten().fold(filter, EnvFilter::add_directive);
    tracing_subscriber::fmt().with_env_filter(filter).init();
    std::panic::set_hook(Box::new(|info| {
        let thread = std::thread::current();
        fatal(&format!("thread {} {info}", thread.name().unwrap_or("unnamed")))
    }));
    serve::run(args)
}

fn skip_vulkan_device_selection_layer() {
    if std::env::var_os("NODEVICE_SELECT").is_none() {
        unsafe { std::env::set_var("NODEVICE_SELECT", "1") };
    }
}

pub(crate) fn fatal(reason: &str) -> ! {
    tracing::error!("{reason}; exiting so that the service manager restarts the daemon");
    quit(70)
}

pub(crate) fn quit(status: i32) -> ! {
    if status == 0 {
        wprs_scene::end_trials();
    }
    unsafe { libc::_exit(status) }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    extern "C" fn hang() {
        std::thread::sleep(Duration::from_secs(600));
    }

    #[test]
    fn a_fatal_error_exits_at_once_even_when_exit_handlers_hang() {
        if std::env::var_os("WPRS_FATAL_CHILD").is_some() {
            unsafe { libc::atexit(hang) };
            super::fatal("a test failure");
        }
        let name = "tests::a_fatal_error_exits_at_once_even_when_exit_handlers_hang";
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture"])
            .env("WPRS_FATAL_CHILD", "1")
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        let status = loop {
            match child.try_wait().unwrap() {
                Some(status) => break status,
                None if Instant::now() > deadline => {
                    child.kill().unwrap();
                    panic!("the daemon hung while exiting");
                }
                None => std::thread::sleep(Duration::from_millis(20)),
            }
        };
        assert_eq!(status.code(), Some(70));
    }
}
