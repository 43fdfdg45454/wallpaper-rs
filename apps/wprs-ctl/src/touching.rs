use crate::editing::change;
use clap::Args;
use std::path::PathBuf;
use wprs_core::edit::Action;

#[derive(Args, Debug)]
pub struct Brush {
    #[arg(help = "Image file")]
    path: PathBuf,
    #[arg(value_name = "X,Y;X,Y;...", help = "Stroke of 1 to 1024 points, in fractions of the image")]
    points: Option<String>,
    #[arg(long, conflicts_with = "lower", help = "Raise the depth towards the viewer, the default")]
    raise: bool,
    #[arg(long, help = "Lower the depth, away from the viewer")]
    lower: bool,
    #[arg(long, help = "Depth added at the middle of the brush, 0 to 1, 0.1 by default")]
    by: Option<f32>,
    #[arg(long, help = "Radius of the brush as a fraction of the longer side, 0.05 by default")]
    radius: Option<f32>,
    #[arg(long, help = "How much of the brush fades towards its edge, 0 hard to 1 soft, 0.5 by default")]
    falloff: Option<f32>,
    #[arg(long, help = "Remove all strokes before painting this one")]
    clear: bool,
}

#[derive(Args, Debug)]
pub struct Cuts {
    #[arg(help = "Image file")]
    path: PathBuf,
    #[arg(
        value_name = "L,L,...",
        required_unless_present = "auto",
        help = "1 to 5 growing depth levels between 0 and 1 where one layer ends and the next begins"
    )]
    cuts: Option<String>,
    #[arg(long, conflicts_with = "cuts", help = "Go back to cuts found from the depth of the picture")]
    auto: bool,
}

#[derive(Args, Debug)]
pub struct Marks {
    #[arg(help = "Image file")]
    path: PathBuf,
    #[arg(value_name = "X,Y;X,Y;...", help = "Stroke of 1 to 1024 points, in fractions of the image")]
    points: Option<String>,
    #[arg(
        long,
        conflicts_with = "protect",
        help = "Fill the area behind near objects even where it is not within reach"
    )]
    refill: bool,
    #[arg(long, help = "Never fill the area")]
    protect: bool,
    #[arg(long, help = "Radius of the brush as a fraction of the longer side, 0.05 by default")]
    radius: Option<f32>,
    #[arg(long, help = "How much of the brush fades towards its edge, 0 hard to 1 soft, 0.5 by default")]
    falloff: Option<f32>,
    #[arg(long, help = "Remove all marks before painting this one, or alone to remove them")]
    clear: bool,
}

pub fn brush(args: Brush) -> anyhow::Result<()> {
    let Brush { path, points, raise, lower, by, radius, falloff, clear } = args;
    change(&path, Action::DepthBrush { points, by, raise, lower, radius, falloff, clear })
}

pub fn cuts(args: Cuts) -> anyhow::Result<()> {
    let Cuts { path, cuts, auto } = args;
    change(&path, Action::DepthCuts { cuts, auto })
}

pub fn marks(args: Marks) -> anyhow::Result<()> {
    let Marks { path, points, refill, protect, radius, falloff, clear } = args;
    change(&path, Action::FillMask { points, refill, protect, radius, falloff, clear })
}
