use clap::{ArgAction, Subcommand};
use serde_json::json;
use wprs_editor::actions::CanvasEdit as C;

#[derive(Subcommand, Debug)]
pub enum Backdrop {
    #[command(
        about = "Let the bright parts of the scene glow, or --off; a new glow starts from the defaults"
    )]
    Bloom {
        #[arg(long, help = "Brightness from 0 to 4 above which it glows")]
        threshold: Option<f64>,
        #[arg(long, help = "From 0 to 8")]
        strength: Option<f64>,
        #[arg(long, help = "Pixels from 0 to 128")]
        radius: Option<f64>,
        #[arg(long, help = "Color as #rrggbb")]
        tint: Option<String>,
        #[arg(
            long,
            value_name = "THRESHOLD,STRENGTH",
            help = "Glare of colors brighter than white on HDR screens, or none"
        )]
        hdr: Option<String>,
        #[arg(long, help = "Remove the glow")]
        off: bool,
    },
    #[command(about = "Move layers in depth with the pointer or the tilt of a phone, or --off")]
    Parallax {
        #[arg(long, help = "From 0 to 4")]
        amount: Option<f64>,
        #[arg(long, help = "Seconds from 0 to 2 it takes to follow")]
        delay: Option<f64>,
        #[arg(long, help = "Share of the pointer it follows, from 0 to 1")]
        pointer: Option<f64>,
        #[arg(long, action = ArgAction::Set, help = "true follows the tilt of a phone")]
        gyro: Option<bool>,
        #[arg(long, help = "Remove the parallax of the canvas")]
        off: bool,
    },
}

pub(crate) fn edit(what: Backdrop) -> C {
    match what {
        Backdrop::Bloom { threshold, strength, radius, tint, hdr, off } => {
            let hdr = hdr.map(|h| crate::sowed::pair(&h).map_or(json!(h), |p| json!(p)));
            C::Bloom { threshold, strength, radius, tint, hdr, off }
        }
        Backdrop::Parallax { amount, delay, pointer, gyro, off } => {
            C::Parallax { amount, delay, pointer, gyro, off }
        }
    }
}
