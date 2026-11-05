use crate::sowed::pair;
use clap::{Args, Subcommand};

const REPLACE: &str = "Replace the force at this place instead of adding one, keeping its window of the life";

#[derive(Subcommand, Debug)]
pub enum Force {
    #[command(about = "Pull the particles in a direction, in pixels a second squared")]
    Gravity {
        #[arg(value_name = "X,Y", value_parser = pair, allow_hyphen_values = true)]
        pull: [f64; 2],
        #[arg(long, help = REPLACE)]
        index: Option<usize>,
    },
    #[command(about = "Slow the particles down, a share of their speed a second")]
    Drag {
        #[arg(allow_negative_numbers = true)]
        amount: f64,
        #[arg(long, help = REPLACE)]
        index: Option<usize>,
    },
    #[command(about = "Push the particles along a moving noise")]
    Turbulence {
        #[arg(allow_negative_numbers = true, long, help = "Pixels a second squared")]
        strength: f64,
        #[arg(
            allow_negative_numbers = true,
            long,
            help = "Size of the noise, as cycles a pixel such as 0.004"
        )]
        scale: f64,
        #[arg(allow_negative_numbers = true, long, help = "How fast the noise moves")]
        speed: Option<f64>,
        #[arg(allow_negative_numbers = true, long, help = "Times the pace of the noise, from 0 to 100")]
        time_scale: Option<f64>,
        #[arg(long, value_name = "MIN,MAX", value_parser = pair, allow_hyphen_values = true, help = "Share of the noise from 0 to 1 that pushes; the rest is still")]
        mask: Option<[f64; 2]>,
        #[arg(long, help = REPLACE)]
        index: Option<usize>,
    },
    #[command(about = "Make the position, alpha or size of the particles swing")]
    Oscillate {
        #[arg(value_parser = ["position", "alpha", "size"])]
        what: String,
        #[arg(allow_hyphen_values = true, long, value_name = "MIN,MAX", value_parser = pair, help = "Swings a second")]
        freq: [f64; 2],
        #[arg(long, value_name = "MIN,MAX", value_parser = pair, allow_hyphen_values = true, help = "Reach of the swing")]
        scale: [f64; 2],
        #[arg(long, value_name = "MIN,MAX", value_parser = pair, allow_hyphen_values = true, help = "Starting phase")]
        phase: Option<[f64; 2]>,
        #[arg(long, help = REPLACE)]
        index: Option<usize>,
    },
    #[command(about = "Pull the particles towards a point, a control point or the pointer")]
    Attractor {
        #[arg(allow_hyphen_values = true, help = "X,Y in pixels of the layer, p0 to p7, or cursor")]
        target: String,
        #[arg(long, allow_hyphen_values = true, help = "Pixels a second squared; negative pushes away")]
        strength: f64,
        #[arg(allow_negative_numbers = true, long, help = "Pixels beyond which it does not pull")]
        radius: Option<f64>,
        #[arg(long, value_parser = ["constant", "linear", "inverse_square"])]
        falloff: Option<String>,
        #[arg(long, help = REPLACE)]
        index: Option<usize>,
    },
    #[command(about = "Remove the force at INDEX")]
    Remove { index: usize },
    #[command(flatten)]
    Pull(crate::pulling::Pull),
}

#[derive(Args, Debug)]
pub struct Kin {
    #[arg(help = "The child system")]
    pub system: String,
    #[arg(long, value_parser = ["static", "spawn", "death", "follow"], help = "When a particle starts it")]
    pub on: String,
    #[arg(allow_negative_numbers = true, long, help = "Chance from 0 to 1 that a particle starts it")]
    pub probability: Option<f64>,
    #[arg(long, help = "Most particles of the child at a time for each parent particle")]
    pub max: Option<u32>,
    #[arg(long, value_name = "X,Y", value_parser = pair, allow_hyphen_values = true, help = "Pixels from the parent particle")]
    pub offset: Option<[f64; 2]>,
    #[arg(long, allow_hyphen_values = true, help = "Degrees turned from the parent particle")]
    pub angle: Option<f64>,
    #[arg(allow_negative_numbers = true, long, help = "Size of the child from 0 to 100")]
    pub scale: Option<f64>,
    #[arg(long, help = "Start from the control points of the parent particle")]
    pub points_from: bool,
}

#[derive(Subcommand, Debug)]
pub enum Child {
    #[command(about = "Add a child system")]
    Add(Kin),
    #[command(about = "Replace the child system at INDEX")]
    Set {
        index: usize,
        #[command(flatten)]
        kin: Kin,
    },
    #[command(about = "Remove the child system at INDEX")]
    Remove { index: usize },
}
