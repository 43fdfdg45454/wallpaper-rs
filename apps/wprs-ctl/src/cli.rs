use crate::library::Lib;
use crate::pack::Pack;
use crate::pkg::Pkg;
use crate::playlists::Playlist;
use crate::profiles::{Preset, Profile};
use crate::providers::Provider;
use crate::remote::Remote;
use crate::repo::Repo;
use clap::{Args, Parser, Subcommand};
use wprs_core::Fit;

pub const OUTPUT: &str = "Output name such as DP-1; every output when omitted";
pub const FIT: &str = "Placement: fill, fit, stretch, center or tile";
pub const JSON: &str = "Print the daemon reply as JSON";
pub const ITEM: &str = "Item: a file path, a uid or the last 6 or more characters of one";

#[derive(Parser, Debug)]
#[command(name = "wprs-ctl", version, about = "Control the wallpaper-rs daemon")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Args, Debug)]
pub struct Out {
    #[arg(short, long, default_value = "", help = OUTPUT)]
    pub output: String,
}

#[derive(Args, Debug)]
pub struct Wait {
    #[arg(long, help = "Wait for the job, showing its progress, and print the finished job")]
    pub wait: bool,
    #[arg(long, help = "With --wait, print the finished job as JSON")]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct Page {
    #[arg(long, default_value_t = 0, help = "Number of entries to skip")]
    pub offset: u32,
    #[arg(long, default_value_t = 50, help = "Number of entries to print, at most 500")]
    pub limit: u32,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    #[command(about = "Set the wallpaper: an image or a video")]
    Set {
        #[arg(help = "Local image or video file, as a path or a file:// URI")]
        path: String,
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
        #[arg(short, long, help = FIT)]
        fit: Option<Fit>,
    },
    #[command(about = "Change only the placement of the current wallpaper")]
    Fit {
        #[arg(help = FIT)]
        fit: Fit,
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
    },
    #[command(about = "Set the color shown around and behind the wallpaper")]
    Background {
        #[arg(help = "Color as #rrggbb")]
        color: String,
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
    },
    #[command(about = "Change a setting, or list them all without arguments")]
    Config {
        #[arg(help = "Setting such as transition.kind; see the list without arguments")]
        key: Option<String>,
        #[arg(help = "New value of the setting")]
        value: Option<String>,
        #[arg(long, help = "Print the settings as a JSON object")]
        json: bool,
    },
    #[command(about = "Re-read the configuration file")]
    Reload,
    #[command(about = "Print the daemon state as JSON")]
    State {
        #[arg(long, help = "Accepted for symmetry: the state is always JSON")]
        json: bool,
    },
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
    #[command(about = "Describe the settings, rules, provider kinds and search keys of the daemon")]
    Schema {
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Print changes and job events as they happen")]
    Watch {
        #[arg(long, help = "Print one JSON object per line")]
        json: bool,
    },
    #[command(about = "Wait for a change and print the new serial and the changed topics")]
    Wait {
        #[arg(long, help = "Serial to wait from; the current one when omitted")]
        serial: Option<u64>,
        #[arg(long, default_value_t = 20000, help = "Milliseconds to wait, at most 20000")]
        timeout: u32,
        #[arg(long, help = "Print the result as JSON")]
        json: bool,
    },
    #[command(about = "List running, queued and recently finished jobs")]
    Jobs {
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Show a job")]
    Job {
        id: u64,
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Cancel a queued or running job")]
    Cancel { id: u64 },
    #[command(subcommand, about = "Library folders, items, tags and thumbnails")]
    Lib(Lib),
    #[command(subcommand, about = "Placement presets of items, and global presets")]
    Preset(Preset),
    #[command(subcommand, about = "Playlists and their rotation")]
    Playlist(Playlist),
    #[command(about = "Show the next item of the playlist of an output")]
    Next(Out),
    #[command(about = "Show the previous item of the playlist of an output")]
    Prev(Out),
    #[command(subcommand, about = "Profiles: saved assignments of every output")]
    Profile(Profile),
    #[command(subcommand, about = "Online sources of wallpapers")]
    Provider(Provider),
    #[command(subcommand, about = "Package repositories added to this computer")]
    Remote(Remote),
    #[command(subcommand, about = "Packages of the added repositories")]
    Pkg(Pkg),
    #[command(subcommand, about = "Build, sign and check .wpak packages; needs no daemon")]
    Pack(Pack),
    #[command(subcommand, about = "Operate a package repository; needs no daemon")]
    Repo(Repo),
}
