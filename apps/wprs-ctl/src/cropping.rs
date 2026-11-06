use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct CropArgs {
    #[arg(help = "Image file")]
    pub path: PathBuf,
    #[arg(
        value_name = "X,Y,W,H",
        help = "Rectangle in fractions of the image, none for all of it; kept when omitted"
    )]
    pub rect: Option<String>,
    #[arg(
        long,
        value_name = "W:H",
        help = "Shape such as 16:9 for the rectangle, which keeps its center; the largest one for the whole picture"
    )]
    pub aspect: Option<String>,
    #[arg(long, help = "Clockwise rotation: 0, 90, 180 or 270")]
    pub rotate: Option<u16>,
    #[arg(long, allow_hyphen_values = true, help = "Degrees between -45 and 45")]
    pub straighten: Option<f32>,
    #[arg(
        long, value_parser = ["none", "h", "v", "hv"],
        help = "Flip horizontally (h), vertically (v), both (hv) or not (none)"
    )]
    pub flip: Option<String>,
    #[arg(short, long, help = "Output name or monitor identity that shows only this rectangle")]
    pub output: Option<String>,
}
