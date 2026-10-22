use clap::{Parser, Subcommand};
use std::path::PathBuf;
use wprs_core::dbus::BUS_NAME;
use zbus::fdo;

pub fn explain(error: zbus::Error) -> anyhow::Error {
    let not_running = || anyhow::anyhow!("wprsd is not running ({} is not on the session bus)", BUS_NAME);
    match error {
        zbus::Error::FDO(e) => match *e {
            fdo::Error::ServiceUnknown(_) | fdo::Error::NameHasNoOwner(_) => not_running(),
            fdo::Error::Failed(message) | fdo::Error::InvalidArgs(message) => anyhow::anyhow!(message),
            other => other.into(),
        },
        zbus::Error::MethodError(name, _, _) if name.as_str().ends_with("ServiceUnknown") => not_running(),
        zbus::Error::MethodError(_, Some(message), _) => anyhow::anyhow!(message),
        other => other.into(),
    }
}

#[derive(Parser)]
#[command(name = "wprs-ctl", version, about = "Control the wallpaper-rs daemon")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Set the wallpaper: an image or a video")]
    Set {
        #[arg(help = "Image or video file")]
        path: PathBuf,
        #[arg(short, long, default_value = "", help = "Output name such as DP-1; every output when omitted")]
        output: String,
        #[arg(short, long, help = "Placement: fill, fit, stretch, center or tile")]
        fit: Option<wprs_core::Fit>,
    },
    #[command(about = "Change only the placement of the current wallpaper")]
    Fit {
        #[arg(help = "Placement: fill, fit, stretch, center or tile")]
        fit: wprs_core::Fit,
        #[arg(short, long, default_value = "", help = "Output name such as DP-1; every output when omitted")]
        output: String,
    },
    #[command(about = "Set the color shown around and behind the wallpaper")]
    Background {
        #[arg(help = "Color as #rrggbb")]
        color: String,
        #[arg(short, long, default_value = "", help = "Output name such as DP-1; every output when omitted")]
        output: String,
    },
    #[command(about = "Change a setting, or list them all without arguments")]
    Config {
        #[arg(help = "Setting such as transition.kind; see the list without arguments")]
        key: Option<String>,
        #[arg(help = "New value of the setting")]
        value: Option<String>,
    },
    #[command(about = "Re-read the configuration file")]
    Reload,
    #[command(about = "Print the daemon state as JSON")]
    State,
    #[command(about = "Pause animated wallpapers")]
    Pause,
    #[command(about = "Resume animated wallpapers")]
    Resume,
    #[command(about = "Pause or resume animated wallpapers")]
    Toggle,
    #[command(about = "Set the volume of video wallpapers, 0 to 100, and unmute them")]
    Volume {
        #[arg(value_parser = clap::value_parser!(u8).range(0..=100))]
        percent: u8,
    },
    #[command(about = "Mute video wallpapers")]
    Mute,
    #[command(about = "Unmute video wallpapers")]
    Unmute,
    #[command(about = "Create the stream of an output and print its node id and serial")]
    Stream {
        output: String,
        width: u32,
        height: u32,
        #[arg(long, help = "Keep the stream only until this command is interrupted, like a desktop client")]
        wait: bool,
    },
    #[command(about = "Release a stream created with the stream command")]
    Release { output: String },
    #[command(about = "Report whether a window covers an output")]
    Covered {
        output: String,
        #[arg(action = clap::ArgAction::Set)]
        covered: bool,
    },
}
