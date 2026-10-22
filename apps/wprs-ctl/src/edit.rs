use crate::editing::{apply, change, export, open, preview};
use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;
use wprs_core::edit::{self, Action};

const MASK: &str = "Mask name, or linear:X0,Y0,X1,Y1, radial:X,Y,RX,RY[,FEATHER] or luminance:LO,HI[,SOFT]";

#[derive(Subcommand, Debug)]
pub enum Edits {
    #[command(about = "Print the edit of an image as JSON")]
    Show { path: PathBuf },
    #[command(
        about = "Set ev, temperature, tint, contrast, saturation, vibrance or a path such as ops.ov.opacity"
    )]
    Set {
        path: PathBuf,
        #[arg(value_name = "KEY=VALUE", required = true)]
        changes: Vec<String>,
    },
    #[command(about = "Crop, rotate, straighten or flip the image, or crop it for one output")]
    Crop {
        path: PathBuf,
        #[arg(
            value_name = "X,Y,W,H",
            help = "Rectangle in fractions of the image, none for all of it; kept when omitted"
        )]
        rect: Option<String>,
        #[arg(
            long,
            value_name = "W:H",
            help = "Shape such as 16:9 for the rectangle, which keeps its center; the largest one for the whole picture"
        )]
        aspect: Option<String>,
        #[arg(long, help = "Clockwise rotation: 0, 90, 180 or 270")]
        rotate: Option<u16>,
        #[arg(long, allow_hyphen_values = true, help = "Degrees between -45 and 45")]
        straighten: Option<f32>,
        #[arg(long, value_parser = ["none", "h", "v", "hv"])]
        flip: Option<String>,
        #[arg(short, long, help = "Output name or monitor identity that shows only this rectangle")]
        output: Option<String>,
    },
    #[command(about = "Set a tone curve through X,Y points between 0 and 1, or clear it without points")]
    Curve {
        path: PathBuf,
        #[arg(value_name = "X,Y")]
        points: Vec<String>,
        #[arg(long, value_parser = ["rgb", "r", "g", "b"], help = "rgb when omitted")]
        channel: Option<String>,
        #[arg(long, help = "cu when omitted")]
        id: Option<String>,
    },
    #[command(about = "Blur the image; the radius is in pixels of a picture 1000 pixels long")]
    Blur {
        path: PathBuf,
        radius: f32,
        #[arg(long, help = MASK)]
        mask: Option<String>,
        #[arg(long, help = "bl when omitted")]
        id: Option<String>,
    },
    #[command(about = "Darken (negative amount) or lighten the edges")]
    Vignette {
        path: PathBuf,
        #[arg(allow_hyphen_values = true)]
        amount: f32,
        #[arg(long, help = "0.5 for a new vignette, otherwise kept when omitted")]
        midpoint: Option<f32>,
        #[arg(long, help = "0 for a new vignette, otherwise kept when omitted")]
        roundness: Option<f32>,
        #[arg(long, help = "0.5 for a new vignette, otherwise kept when omitted")]
        feather: Option<f32>,
        #[arg(long, help = "vi when omitted")]
        id: Option<String>,
    },
    #[command(about = "Lay a color over the image")]
    Overlay {
        path: PathBuf,
        #[arg(help = "Color as #rrggbb")]
        color: String,
        #[arg(long, value_parser = ["normal", "multiply", "screen", "overlay", "soft-light", "color"], help = "normal for a new overlay, otherwise kept when omitted")]
        blend: Option<String>,
        #[arg(long, help = "1 for a new overlay, otherwise kept when omitted")]
        opacity: Option<f32>,
        #[arg(long, help = MASK)]
        mask: Option<String>,
        #[arg(long, help = "ov when omitted")]
        id: Option<String>,
    },
    #[command(about = "Add or replace a named mask, or remove it with none")]
    Mask {
        path: PathBuf,
        name: String,
        #[arg(help = MASK)]
        spec: String,
        #[arg(long)]
        invert: bool,
    },
    #[command(about = "Put operations first, in this order")]
    Order {
        path: PathBuf,
        #[arg(value_name = "ID", required = true)]
        ids: Vec<String>,
    },
    #[command(about = "Spread the image over the screens with the span fit")]
    Span {
        path: PathBuf,
        #[arg(long, help = "1 or more")]
        zoom: Option<f32>,
        #[arg(long, value_name = "X,Y", help = "Center of the shown part, in fractions of the image")]
        center: Option<String>,
    },
    #[command(about = "Remove operations, or the whole edit without ids")]
    Reset {
        path: PathBuf,
        #[arg(value_name = "ID")]
        ids: Vec<String>,
    },
    #[command(about = "Replace the edit with a JSON file, or - for standard input")]
    Apply { path: PathBuf, file: String },
    #[command(about = "Show changes on the desktop for 60 s without saving them")]
    Preview {
        path: PathBuf,
        #[arg(value_name = "KEY=VALUE")]
        changes: Vec<String>,
        #[arg(long, help = "End the preview")]
        stop: bool,
    },
    #[command(about = "Render the edited image to a PNG or JPEG file; needs no daemon")]
    Export {
        path: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
        #[arg(long, default_value_t = 90, value_parser = clap::value_parser!(u8).range(1..=100))]
        quality: u8,
        #[arg(
            long,
            value_name = "WxH",
            help = "Size of the file; the size of the edited image when omitted"
        )]
        size: Option<String>,
    },
}

fn action(command: Edits) -> Option<(PathBuf, Action)> {
    Some(match command {
        Edits::Set { path, changes } => (path, Action::Set { changes }),
        Edits::Crop { path, rect, aspect, rotate, straighten, flip, output } => {
            (path, Action::Crop { rect, aspect, rotate, straighten, flip, output })
        }
        Edits::Curve { path, points, channel, id } => (path, Action::Curve { points, channel, id }),
        Edits::Blur { path, radius, mask, id } => (path, Action::Blur { radius: Some(radius), mask, id }),
        Edits::Vignette { path, amount, midpoint, roundness, feather, id } => {
            (path, Action::Vignette { amount: Some(amount), midpoint, roundness, feather, id })
        }
        Edits::Overlay { path, color, blend, opacity, mask, id } => {
            (path, Action::Overlay { color: Some(color), blend, opacity, mask, id })
        }
        Edits::Mask { path, name, spec, invert } => (path, Action::Mask { name, spec, invert }),
        Edits::Order { path, ids } => (path, Action::Order { ids }),
        Edits::Span { path, zoom, center } => (path, Action::Span { zoom, center }),
        Edits::Reset { path, ids } => (path, Action::Reset { ids }),
        Edits::Show { .. } | Edits::Apply { .. } | Edits::Preview { .. } | Edits::Export { .. } => {
            return None;
        }
    })
}

pub fn run(command: Edits) -> Result<()> {
    match command {
        Edits::Show { path } => print!("{}", open(&path)?.to_json()),
        Edits::Reset { path, ids } if ids.is_empty() => {
            edit::remove(&path, &edit::dir()).map_err(anyhow::Error::msg)?;
        }
        Edits::Apply { path, file } => apply(&path, &file)?,
        Edits::Preview { path, changes, stop } => preview(&path, changes, stop)?,
        Edits::Export { path, out, quality, size } => export(&path, &out, quality, size.as_deref())?,
        command => action(command).map_or(Ok(()), |(path, action)| change(&path, action))?,
    }
    Ok(())
}
