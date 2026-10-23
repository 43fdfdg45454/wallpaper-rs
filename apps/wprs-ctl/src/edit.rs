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
    #[command(
        about = "Adjust the depth map of an image, or generate, import or export it",
        args_conflicts_with_subcommands = true,
        subcommand_negates_reqs = true
    )]
    Depth {
        #[arg(required = true)]
        path: Option<PathBuf>,
        #[arg(long, help = "Level of the nearest point, 0 to 1, 1 by default")]
        near: Option<f32>,
        #[arg(long, help = "Level of the farthest point, 0 to 1, 0 by default")]
        far: Option<f32>,
        #[arg(long, help = "Curve between them, 0.2 to 5, 1 by default")]
        gamma: Option<f32>,
        #[arg(long, help = "Blur radius in pixels of the map, 0 to 16, 2 by default")]
        smooth: Option<u32>,
        #[arg(long, help = "Pixels by which near objects grow over their edges, 0 to 16, 3 by default")]
        dilate: Option<u32>,
        #[arg(
            long,
            help = "Short side given to the model: a multiple of 14 from 196 to 1036, 518 by default"
        )]
        input: Option<u32>,
        #[command(subcommand)]
        map: Option<crate::depth::Maps>,
    },
    #[command(about = "Move the image in depth on the desktop; any change turns it on unless --off")]
    Parallax {
        path: PathBuf,
        #[arg(long, value_parser = ["auto", "pointer", "accelerometer"], help = "What moves it; auto when new")]
        source: Option<String>,
        #[arg(long, help = "Largest shift as a fraction of the width, up to 0.03")]
        strength: Option<f32>,
        #[arg(long, help = "Depth that stays still, 0 far to 1 near")]
        focus: Option<f32>,
        #[arg(long, value_parser = ["1", "8", "16", "32"], help = "Quality: 1 shifts, more steps follow edges")]
        steps: Option<String>,
        #[arg(long, value_name = "MS", help = "Time the movement takes to settle, 0 to 2000 ms")]
        smoothing: Option<u32>,
        #[arg(long, help = "Zoom that hides the edges, 1 to 1.1")]
        overscan: Option<f32>,
        #[arg(long, help = "Turn the parallax off and keep its settings")]
        off: bool,
    },
    #[command(about = "Show this picture with its own fit, or with the screen's fit again with none")]
    Fit {
        path: PathBuf,
        #[arg(value_parser = ["fill", "fit", "stretch", "center", "tile", "none"])]
        mode: String,
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
        Edits::Depth { path, near, far, gamma, smooth, dilate, input, map: None } => {
            (path?, Action::Depth { near, far, gamma, smooth, dilate, input })
        }
        Edits::Parallax { path, source, strength, focus, steps, smoothing, overscan, off } => {
            let steps = steps.and_then(|s| s.parse().ok());
            (path, Action::Parallax { source, strength, focus, steps, smoothing, overscan, off })
        }
        Edits::Fit { path, mode } => (path, Action::Fit { mode }),
        Edits::Show { .. }
        | Edits::Apply { .. }
        | Edits::Preview { .. }
        | Edits::Export { .. }
        | Edits::Depth { map: Some(_), .. } => {
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
        Edits::Depth { map: Some(map), .. } => crate::depth::run(map)?,
        command => action(command).map_or(Ok(()), |(path, action)| change(&path, action))?,
    }
    Ok(())
}
