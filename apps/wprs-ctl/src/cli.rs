use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    Stream { output: String, width: u32, height: u32 },
    #[command(about = "Destroy the stream of an output")]
    Release { output: String },
    #[command(about = "Report whether a window covers an output")]
    Covered {
        output: String,
        #[arg(action = clap::ArgAction::Set)]
        covered: bool,
    },
}
