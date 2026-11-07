use clap::Args;
use wprs_core::edit::Action;

#[derive(Args, Debug)]
pub struct Levels {
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
    #[arg(long, help = "Short side given to the model: a multiple of 14 from 196 to 1036, 518 by default")]
    input: Option<u32>,
}

#[derive(Args, Debug)]
pub struct Look {
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
}

impl Levels {
    pub fn action(self) -> Action {
        let Levels { near, far, gamma, smooth, dilate, input } = self;
        Action::Depth { near, far, gamma, smooth, dilate, input }
    }
}

impl Look {
    pub fn action(self) -> Action {
        let Look { source, strength, focus, steps, smoothing, overscan, off } = self;
        let steps = steps.and_then(|s| s.parse().ok());
        Action::Parallax { source, strength, focus, steps, smoothing, overscan, off }
    }
}
