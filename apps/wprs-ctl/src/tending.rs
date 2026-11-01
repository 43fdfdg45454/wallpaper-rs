use clap::{Args, Subcommand};

#[derive(Subcommand, Debug)]
pub enum Collider {
    #[command(
        about = "Add a collider as JSON, such as '{\"bounds\": {\"box\": {\"from\": [0, 0], \"to\": [400, 300], \"inside\": true}}, \"bounce\": 0.5}'"
    )]
    Add { collider: String },
    #[command(about = "Replace the collider at INDEX")]
    Set { index: usize, collider: String },
    #[command(about = "Remove the collider at INDEX")]
    Remove { index: usize },
    #[command(
        about = "Make particles bounce, slide, stop, stick or die where they touch the collider at INDEX"
    )]
    Response {
        index: usize,
        #[arg(value_parser = ["bounce", "slide", "stop", "stick", "kill"])]
        response: String,
        #[arg(long, help = "From 0 to 1, with bounce")]
        bounce: Option<f64>,
        #[arg(long, help = "From 0 to 1, with bounce or slide")]
        friction: Option<f64>,
    },
}

#[derive(Subcommand, Debug)]
pub enum Keep {
    #[command(about = "Keep particles at a distance from TARGET: p0 to p7, cursor or X,Y")]
    Distance {
        #[arg(allow_hyphen_values = true)]
        target: String,
        #[arg(long, default_value_t = 100.0)]
        distance: f64,
        #[arg(long, default_value_t = 4.0, allow_hyphen_values = true)]
        strength: f64,
        #[arg(long, help = "Replace the force at this index instead of adding one")]
        index: Option<usize>,
    },
    #[command(about = "Draw particles to the segment between FROM and TO: p0 to p7, cursor or X,Y")]
    Between {
        #[arg(allow_hyphen_values = true)]
        from: String,
        #[arg(allow_hyphen_values = true)]
        to: String,
        #[arg(long, default_value_t = 4.0, allow_hyphen_values = true)]
        strength: f64,
        #[arg(long, help = "Replace the force at this index instead of adding one")]
        index: Option<usize>,
    },
    #[command(about = "Remove the force at INDEX")]
    Remove { index: usize },
}

#[derive(Args, Debug)]
pub struct Boids {
    #[arg(long, help = "Pixels within which particles are neighbours")]
    pub radius: Option<f64>,
    #[arg(long, help = "From 0 to 10")]
    pub separation: Option<f64>,
    #[arg(long, help = "From 0 to 10")]
    pub alignment: Option<f64>,
    #[arg(long, help = "From 0 to 10")]
    pub cohesion: Option<f64>,
    #[arg(long, help = "Pixels a second, 0 for none")]
    pub max_speed: Option<f64>,
    #[arg(long, help = "Remove the flock")]
    pub off: bool,
}

#[derive(Args, Debug)]
pub struct Rope {
    #[arg(long, help = "Points between two particles, 0 to 7")]
    pub subdivide: Option<u32>,
    #[arg(long, help = "Pixels")]
    pub width: Option<f64>,
    #[arg(long)]
    pub uv_scale: Option<f64>,
    #[arg(long, allow_hyphen_values = true)]
    pub uv_scroll: Option<f64>,
    #[arg(long, help = "A Catmull-Rom curve (true) or straight lines (false)")]
    pub smooth: Option<bool>,
    #[arg(long, help = "A particles layer whose own render changes")]
    pub layer: Option<String>,
    #[arg(long, help = "Remove the rope")]
    pub off: bool,
}

#[derive(Args, Debug)]
pub struct Trail {
    #[arg(long, help = "Steps of 1/60 s, 1 to 64")]
    pub length: Option<u32>,
    #[arg(long, help = "From 0 to 1 of the particle size")]
    pub width: Option<f64>,
    #[arg(long, help = "Positions kept along the length, minus one")]
    pub segments: Option<u32>,
    #[arg(long, help = "Points between two kept positions, 0 to 7")]
    pub subdivide: Option<u32>,
    #[arg(long)]
    pub uv_scale: Option<f64>,
    #[arg(long, allow_hyphen_values = true)]
    pub uv_scroll: Option<f64>,
    #[arg(long, help = "Show the texture along the trail")]
    pub textured: Option<bool>,
    #[arg(long, help = "A particles layer whose own render changes")]
    pub layer: Option<String>,
    #[arg(long, help = "Remove the trail")]
    pub off: bool,
}

#[derive(Subcommand, Debug)]
pub enum Operator {
    #[command(subcommand, about = "Add, set or remove a remap applied every step, with the age input too")]
    Remap(crate::swarming::Remap),
}
