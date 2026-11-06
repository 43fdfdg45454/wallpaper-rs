use crate::author::DIR;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum Swarming {
    #[command(about = "Shape, place, remap and draw a particle system with the tools of the scene editor")]
    Particles {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = "Id of the particle system")]
        system: String,
        #[command(subcommand)]
        what: Particle,
    },
}

#[derive(Subcommand, Debug)]
pub enum Particle {
    #[command(
        about = "Set the shape of an emitter as JSON, such as '{\"around\": {\"at\": \"p1\", \"count\": 12, \"radius\": 40}}'; the next index adds an emitter"
    )]
    Shape {
        #[arg(help = "The emitter, 0 first; the next index adds one")]
        emitter: usize,
        #[arg(help = "The shape as JSON")]
        shape: String,
    },
    #[command(about = "Place, turn, let follow the pointer or remove a control point p0 to p7")]
    ControlPoint {
        #[arg(help = "p0 to p7", value_parser = ["p0", "p1", "p2", "p3", "p4", "p5", "p6", "p7"])]
        point: String,
        #[arg(long, allow_hyphen_values = true, value_name = "X,Y", help = "Pixels of the layer")]
        at: Option<String>,
        #[arg(long, allow_hyphen_values = true, help = "Degrees from -360 to 360")]
        angle: Option<f64>,
        #[arg(long, help = "Follow the pointer, which needs the pointer permission", conflicts_with = "at")]
        cursor: bool,
        #[arg(long, help = "Remove the last control point")]
        remove: bool,
    },
    #[command(subcommand, about = "Add, set or remove a remap of the starting values")]
    Remap(Remap),
    #[command(about = "Cut out the alpha of the texture: START,END,OPACITY, or none")]
    Cutout {
        #[arg(help = "START,END,OPACITY from 0 to 1, or none")]
        range: String,
        #[arg(long, help = "A particles layer whose own render changes")]
        layer: Option<String>,
    },
    #[command(about = "Turn sprites to the screen, along their motion or to a fixed angle, or none")]
    Orient {
        #[arg(help = "screen, velocity, fixed or none", value_parser = ["screen", "velocity", "fixed", "none"])]
        orient: String,
        #[arg(long, allow_hyphen_values = true, help = "Degrees on the screen, with fixed")]
        angle: Option<f64>,
        #[arg(long, help = "A particles layer whose own render changes")]
        layer: Option<String>,
    },
    #[command(about = "Give the particles depth seen with a focal length in pixels")]
    Perspective {
        #[arg(help = "Focal length in pixels", allow_negative_numbers = true, long)]
        focal: Option<f64>,
        #[arg(long, allow_hyphen_values = true, value_name = "MIN,MAX", help = "Starting depth in pixels")]
        depth: Option<String>,
        #[arg(long, allow_hyphen_values = true, value_name = "MIN,MAX", help = "Pixels a second")]
        depth_speed: Option<String>,
        #[arg(long, help = "Remove the perspective and the depths")]
        off: bool,
    },
    #[command(about = "Emit from the opaque pixels of a layer that does not hold the system")]
    EmitFromLayer {
        #[arg(help = "The emitter, 0 first")]
        emitter: usize,
        #[arg(help = "The layer whose opaque pixels emit")]
        layer: String,
        #[arg(allow_negative_numbers = true, long, help = "Alpha from 0 to 1 above which a pixel emits")]
        threshold: Option<f64>,
        #[arg(long, help = "Take the color of the layer")]
        color: bool,
        #[arg(long, help = "Take the motion of the layer")]
        inherit: bool,
        #[arg(allow_negative_numbers = true, long, help = "Pixels births spread over")]
        jitter: Option<f64>,
        #[arg(help = "Read the layer once (static) or each time it changes (on_change)", long, value_parser = ["static", "on_change"])]
        refresh: Option<String>,
    },
    #[command(
        about = "Choose what a child system takes from its parent particle: color, size, alpha, velocity (with its speed in depth when both have perspective)"
    )]
    Inherit {
        #[arg(help = "The child system, 0 first")]
        child: usize,
        #[arg(help = "What it takes: color, size, alpha or velocity", value_parser = ["color", "size", "alpha", "velocity"])]
        take: Vec<String>,
        #[arg(long, help = "Take them every step, for follow children")]
        follow: bool,
    },
    #[command(about = "Start particles in directions turned by a smooth noise, or --off")]
    Turbulent(crate::fielding::Turbulent),
    #[command(about = "Move each birth along a fractal noise, or --off")]
    Offset(crate::fielding::Offset),
    #[command(
        about = "Limit how many particles emitters start each step; no --max-per-step removes the limit"
    )]
    Pace(crate::fielding::Pace),
    #[command(about = "Light the particles with the lights of the scene, with a normal map, or --off")]
    Lit(crate::glowing::Lit),
    #[command(about = "Bend what is behind the particles with a normal map, or --off")]
    Refract(crate::glowing::Bend),
    #[command(about = "Draw with a package effect whose stage is particle, or none")]
    Shader {
        #[arg(help = "Id of a package effect whose stage is particle, or none")]
        shader: String,
        #[arg(long, help = "A particles layer whose own render changes")]
        layer: Option<String>,
    },
    #[command(subcommand, about = "Add, set or remove colliders and choose how particles meet them")]
    Collider(crate::tending::Collider),
    #[command(subcommand, about = "Keep particles at a distance from a point or between two")]
    Keep(crate::tending::Keep),
    #[command(about = "Move particles in a flock, or --off")]
    Boids(crate::tending::Boids),
    #[command(subcommand, about = "Change values of the particles every step")]
    Operator(crate::tending::Operator),
    #[command(about = "Join the particles with a rope in the order they were born, or --off")]
    Rope(crate::tending::Rope),
    #[command(about = "Leave a trail of up to 64 steps behind each particle, or --off")]
    Trail(crate::tending::Trail),
    #[command(about = "Draw particles, ropes and textured trails with an image asset, or none")]
    Texture {
        #[arg(help = "Id of an image asset, or none")]
        texture: String,
        #[arg(long, help = "A particles layer whose own render changes")]
        layer: Option<String>,
    },
    #[command(flatten)]
    Sowing(crate::sowing::Sowing),
    #[command(flatten)]
    Seeding(crate::seeding::Seeding),
    #[command(
        about = "Print the particle budget as JSON: particles alive at a time on the processor, slots by device tier and path, and what each quality level keeps"
    )]
    Budget {
        #[arg(
            help = "Seconds simulated before counting",
            allow_negative_numbers = true,
            long,
            default_value_t = 2.0
        )]
        time: f64,
        #[arg(help = "Count for this quality level only: q0, q1, q2 or q3", long, value_parser = ["q0", "q1", "q2", "q3"])]
        quality: Option<String>,
        #[arg(help = "A particles layer showing the system", long)]
        layer: Option<String>,
    },
    #[command(about = "Print what the editor draws over the preview for the system as JSON")]
    Handles {
        #[arg(help = "A particles layer showing the system", long)]
        layer: Option<String>,
        #[arg(help = "Seconds into the scene", allow_negative_numbers = true, long, default_value_t = 0.0)]
        time: f64,
        #[arg(
            long,
            help = "Seed of the run to draw the rays with, as the editor shows it; the scene seed by default"
        )]
        seed: Option<u64>,
        #[arg(
            long,
            default_value_t = 0.0,
            help = "Sound level from 0 to 1 that moves the phase of rays that follow a band"
        )]
        sound: f32,
    },
}

#[derive(Subcommand, Debug)]
pub enum Remap {
    #[command(
        about = "Add a remap as JSON, such as '{\"input\": \"random\", \"in\": [0, 1], \"output\": \"size\", \"out\": [2, 8]}'"
    )]
    Add {
        #[arg(help = "The remap as JSON")]
        remap: String,
    },
    #[command(about = "Replace the remap at INDEX")]
    Set {
        #[arg(help = "Its place in the list, 0 first")]
        index: usize,
        #[arg(help = "The remap as JSON")]
        remap: String,
    },
    #[command(about = "Remove the remap at INDEX")]
    Remove {
        #[arg(help = "Its place in the list, 0 first")]
        index: usize,
    },
}
