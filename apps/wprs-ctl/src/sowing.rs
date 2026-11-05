use crate::sowed::{pair, sweep};
use clap::{ArgAction, Subcommand};

const MOMENTS: [&str; 4] = ["static", "spawn", "death", "follow"];
const LAYER: &str = "A particles layer whose own render changes";
const RANGE: &str = "MIN,MAX";
pub(crate) const REPLACE: &str = "Write the plain value over keyframes or another source that drives it";

#[derive(Subcommand, Debug)]
pub enum Sowing {
    #[command(
        about = "Create the system without a layer, from the default or as a copy, to start it from another system or draw it later"
    )]
    Create {
        #[arg(long, value_name = "SYSTEM", help = "Copy this system")]
        from: Option<String>,
        #[arg(long, value_name = "SYSTEM", help = "Start the new system from the particles of this one")]
        parent: Option<String>,
        #[arg(long, value_parser = MOMENTS, requires = "parent", help = "When the parent starts it; static when omitted")]
        on: Option<String>,
    },
    #[command(about = "Remove the system; refused while a layer or another system uses it")]
    Delete,
    #[command(about = "Set the capacity, seed, warm-up and space of the system and the rate of its emitters")]
    Settings {
        #[arg(long, help = "Particles alive at a time")]
        capacity: Option<u32>,
        #[arg(long, help = "Seed of the random numbers")]
        seed: Option<u64>,
        #[arg(allow_negative_numbers = true, long, help = "Seconds simulated before the first frame")]
        prewarm: Option<f64>,
        #[arg(long, value_parser = ["local", "world"], help = "local moves the particles with the layer")]
        space: Option<String>,
        #[arg(allow_negative_numbers = true, long, help = "Particles each emitter starts a second")]
        rate: Option<f64>,
        #[arg(long, requires = "rate", help = "Set the rate of this emitter only")]
        emitter: Option<usize>,
        #[arg(long, requires = "rate", help = REPLACE)]
        replace: bool,
    },
    #[command(about = "Set the ranges new particles start from; the next value is picked between the two")]
    Start {
        #[arg(long, value_name = RANGE, value_parser = pair, allow_hyphen_values = true, help = "Seconds")]
        life: Option<[f64; 2]>,
        #[arg(long, value_name = RANGE, value_parser = pair, allow_hyphen_values = true, help = "Pixels a second")]
        speed: Option<[f64; 2]>,
        #[arg(long, value_name = RANGE, value_parser = pair, allow_hyphen_values = true, help = "Degrees of the direction")]
        angle: Option<[f64; 2]>,
        #[arg(long, value_name = RANGE, value_parser = pair, allow_hyphen_values = true, help = "Pixels")]
        size: Option<[f64; 2]>,
        #[arg(long, value_name = RANGE, value_parser = pair, allow_hyphen_values = true, help = "Degrees")]
        rotation: Option<[f64; 2]>,
        #[arg(long, value_name = RANGE, value_parser = pair, allow_hyphen_values = true, help = "Degrees a second")]
        spin: Option<[f64; 2]>,
        #[arg(long, value_name = RANGE, value_parser = pair, allow_hyphen_values = true, help = "Frame of the atlas")]
        frame: Option<[f64; 2]>,
        #[arg(long, value_name = "COLOR,COLOR", value_parser = crate::sowed::colors, help = "Two colors such as #ffffff,#a0c8ff")]
        color: Option<String>,
        #[arg(allow_hyphen_values = true, long, value_name = RANGE, value_parser = pair, help = "From 0 to 1")]
        alpha: Option<[f64; 2]>,
        #[arg(long, value_name = "X,Y:X,Y", value_parser = sweep, allow_hyphen_values = true, help = "Pixels a second, instead of speed and angle")]
        velocity: Option<[[f64; 2]; 2]>,
        #[arg(long, value_name = RANGE, value_parser = pair, allow_hyphen_values = true, help = "Pixels a second away from the center of the emitter")]
        radial: Option<[f64; 2]>,
        #[arg(long, value_parser = ["alpha", "velocity", "radial"], help = "Remove a range")]
        clear: Vec<String>,
    },
    #[command(subcommand, about = "Add, replace or remove the forces that move the particles")]
    Force(crate::forcing::Force),
    #[command(subcommand, about = "Add, replace or remove the systems that start from these particles")]
    Child(crate::forcing::Child),
    #[command(
        about = "Scale the particles over their life with TIME:SCALE keys, the time from 0 to 1, or none"
    )]
    SizeOverLife {
        #[arg(required = true, value_name = "TIME:SCALE", value_parser = crate::sowed::scale_key, help = "TIME from 0 to 1 of the life and SCALE from 0 to 100, or none")]
        keys: Vec<String>,
        #[arg(long, help = LAYER)]
        layer: Option<String>,
    },
    #[command(
        about = "Tint the particles over their life with TIME:COLOR stops, the time from 0 to 1, or none"
    )]
    ColorOverLife {
        #[arg(required = true, value_name = "TIME:COLOR", value_parser = crate::sowed::color_stop, help = "TIME from 0 to 1 of the life and a color such as #ff000080, or none")]
        stops: Vec<String>,
        #[arg(long, help = LAYER)]
        layer: Option<String>,
    },
    #[command(
        about = "Stretch sprites along their motion, pick the frames of the atlas, set its frame rate or let the particles shine above white"
    )]
    Render {
        #[arg(
            long,
            value_name = "TIME[,MIN,MAX]",
            help = "Seconds of motion a sprite stretches over, with optional limits in pixels; 0 stops it"
        )]
        stretch: Option<String>,
        #[arg(long, value_parser = ["time", "life", "random", "none"], help = "How a particle walks the frames of the atlas")]
        frames: Option<String>,
        #[arg(allow_negative_numbers = true, long, help = "Frames a second of the atlas")]
        fps: Option<f64>,
        #[arg(long, action = ArgAction::Set, help = "true blends one frame into the next")]
        mix: Option<bool>,
        #[arg(
            allow_negative_numbers = true,
            long,
            help = "Brightness from 0 to 16; above 1 shines over white"
        )]
        brightness: Option<f64>,
        #[arg(long, help = LAYER)]
        layer: Option<String>,
    },
}
