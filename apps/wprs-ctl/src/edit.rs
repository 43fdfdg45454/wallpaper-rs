use clap::Subcommand;
use std::path::PathBuf;

const MASK: &str = "Mask name, or linear:X0,Y0,X1,Y1, radial:X,Y,RX,RY[,FEATHER], luminance:LO,HI[,SOFT], depth:LO,HI[,SOFT], or a brush stroke added to the mask, brush:RADIUS,HARDNESS:X,Y;X,Y;... or erase:RADIUS,HARDNESS:X,Y;...";

pub use crate::editing::run;

#[derive(Subcommand, Debug)]
pub enum Edits {
    #[command(about = "Print the edit of an image as JSON")]
    Show {
        #[arg(help = "Image file")]
        path: PathBuf,
    },
    #[command(
        about = "Set ev, temperature, tint, contrast, saturation, vibrance or a path such as ops.ov.opacity"
    )]
    Set {
        #[arg(help = "Image file")]
        path: PathBuf,
        #[arg(value_name = "KEY=VALUE", required = true, help = "Value to set, such as ev=0.5")]
        changes: Vec<String>,
    },
    #[command(about = "Crop, rotate, straighten or flip the image, or crop it for one output")]
    Crop(Box<crate::cropping::CropArgs>),
    #[command(about = "Set a tone curve through X,Y points between 0 and 1, or clear it without points")]
    Curve {
        #[arg(help = "Image file")]
        path: PathBuf,
        #[arg(value_name = "X,Y", help = "Point of the curve, both between 0 and 1")]
        points: Vec<String>,
        #[arg(long, value_parser = ["rgb", "r", "g", "b"], help = "rgb when omitted")]
        channel: Option<String>,
        #[arg(long, help = "cu when omitted")]
        id: Option<String>,
    },
    #[command(about = "Blur the image; the radius is in pixels of a picture 1000 pixels long")]
    Blur {
        #[arg(help = "Image file")]
        path: PathBuf,
        #[arg(help = "Blur radius")]
        radius: f32,
        #[arg(long, help = MASK)]
        mask: Option<String>,
        #[arg(long, help = "bl when omitted")]
        id: Option<String>,
    },
    #[command(about = "Darken (negative amount) or lighten the edges")]
    Vignette {
        #[arg(help = "Image file")]
        path: PathBuf,
        #[arg(allow_hyphen_values = true, help = "Strength from -1 (darker edges) to 1 (lighter edges)")]
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
        #[arg(help = "Image file")]
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
    #[command(
        about = "Add or replace a named mask, paint or erase a stroke of a brush mask, or remove it with none"
    )]
    Mask {
        #[arg(help = "Image file")]
        path: PathBuf,
        #[arg(help = "Name of the mask")]
        name: String,
        #[arg(help = MASK)]
        spec: String,
        #[arg(long, help = "Invert the mask; with a brush stroke, the whole brush mask")]
        invert: bool,
    },
    #[command(about = "Put operations first, in this order")]
    Order {
        #[arg(help = "Image file")]
        path: PathBuf,
        #[arg(value_name = "ID", required = true, help = "Id of an operation")]
        ids: Vec<String>,
    },
    #[command(about = "Spread the image over the screens with the span fit")]
    Span {
        #[arg(help = "Image file")]
        path: PathBuf,
        #[arg(long, help = "1 or more")]
        zoom: Option<f32>,
        #[arg(long, value_name = "X,Y", help = "Center of the shown part, in fractions of the image")]
        center: Option<String>,
    },
    #[command(about = "Remove operations, or the whole edit without ids")]
    Reset {
        #[arg(help = "Image file")]
        path: PathBuf,
        #[arg(value_name = "ID", help = "Id of an operation to remove; the whole edit when left out")]
        ids: Vec<String>,
    },
    #[command(
        about = "Adjust the depth map of an image, or generate, import or export it",
        args_conflicts_with_subcommands = true,
        subcommand_negates_reqs = true
    )]
    Depth {
        #[arg(required = true, help = "Image file")]
        path: Option<PathBuf>,
        #[command(flatten)]
        levels: crate::depth::Levels,
        #[command(subcommand)]
        map: Option<crate::depth::Maps>,
    },
    #[command(about = "Move the image in depth on the desktop; any change turns it on unless --off")]
    Parallax {
        #[arg(help = "Image file")]
        path: PathBuf,
        #[command(flatten)]
        look: crate::depth::Look,
    },
    #[command(about = "Show this picture with its own fit, or with the screen's fit again with none")]
    Fit {
        #[arg(help = "Image file")]
        path: PathBuf,
        #[arg(
            value_parser = ["fill", "fit", "stretch", "center", "tile", "none"],
            help = "Fit of this picture, or none for the screen's fit"
        )]
        mode: String,
    },
    #[command(about = "Replace the edit with a JSON file, or - for standard input")]
    Apply {
        #[arg(help = "Image file")]
        path: PathBuf,
        #[arg(help = "JSON file of the edit, or - for standard input")]
        file: String,
    },
    #[command(about = "Show changes on the desktop for 60 s without saving them")]
    Preview {
        #[arg(help = "Image file")]
        path: PathBuf,
        #[arg(value_name = "KEY=VALUE", help = "Value to show, such as ev=0.5")]
        changes: Vec<String>,
        #[arg(long, help = "End the preview")]
        stop: bool,
    },
    #[command(about = "Go back to the edit before the last change")]
    Undo {
        #[arg(help = "Image file")]
        path: PathBuf,
    },
    #[command(about = "Make again the change that undo went back from")]
    Redo {
        #[arg(help = "Image file")]
        path: PathBuf,
    },
    #[command(about = "Render the edited image to a PNG, JPEG, WebP or AVIF file; needs no daemon")]
    Export {
        #[arg(help = "Image file")]
        path: PathBuf,
        #[arg(short, long, help = "The extension chooses the format: png, jpg, webp (lossless) or avif")]
        out: PathBuf,
        #[arg(long, help = "Render the picture without its operations, to compare")]
        before: bool,
        #[arg(
            long, default_value_t = 90, value_parser = clap::value_parser!(u8).range(1..=100),
            help = "Quality of a lossy file, 1 to 100"
        )]
        quality: u8,
        #[arg(
            long,
            value_name = "WxH",
            help = "Size of the file; the size of the edited image when omitted"
        )]
        size: Option<String>,
    },
}
