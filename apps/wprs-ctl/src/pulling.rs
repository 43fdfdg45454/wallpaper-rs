use crate::sowed::pair;
use clap::Subcommand;
use serde_json::{Value, json};

const REPLACE: &str = "Replace the force at this place instead of adding one";
const FALLOFF: [&str; 3] = ["constant", "linear", "inverse_square"];

#[derive(Subcommand, Debug)]
pub enum Pull {
    #[command(about = "Shake the particles at random, in pixels a second squared")]
    Noise {
        #[arg(allow_negative_numbers = true)]
        amount: f64,
        #[arg(long, help = REPLACE)]
        index: Option<usize>,
    },
    #[command(about = "Keep the particles below a speed, in pixels a second")]
    Cap {
        #[arg(allow_negative_numbers = true)]
        speed: f64,
        #[arg(long, help = REPLACE)]
        index: Option<usize>,
    },
    #[command(about = "Turn the particles faster, in degrees a second squared; negative turns the other way")]
    Torque {
        #[arg(allow_negative_numbers = true)]
        amount: f64,
        #[arg(long, help = REPLACE)]
        index: Option<usize>,
    },
    #[command(about = "Slow the turning of the particles down, a share of their spin a second")]
    SpinDrag {
        #[arg(allow_negative_numbers = true)]
        amount: f64,
        #[arg(long, help = REPLACE)]
        index: Option<usize>,
    },
    #[command(about = "Swirl the particles around a point, a control point or the pointer")]
    Vortex {
        #[arg(allow_hyphen_values = true, help = "X,Y in pixels of the layer, p0 to p7, or cursor")]
        center: String,
        #[arg(
            long,
            allow_negative_numbers = true,
            help = "Pixels a second squared; negative turns the other way"
        )]
        strength: f64,
        #[arg(long, allow_negative_numbers = true, help = "Pixels beyond which it does not swirl")]
        radius: Option<f64>,
        #[arg(long, value_parser = FALLOFF, help = "How the pull weakens with distance")]
        falloff: Option<String>,
        #[arg(
            long,
            allow_negative_numbers = true,
            help = "Pull towards the center, pixels a second squared"
        )]
        inner: Option<f64>,
        #[arg(
            long,
            allow_negative_numbers = true,
            help = "Push away from the center, pixels a second squared"
        )]
        outer: Option<f64>,
        #[arg(long, value_name = "RADIUS,WIDTH,PULL", value_parser = crate::seeding::three, help = "Hold the particles on a ring of this radius and width with this pull")]
        ring: Option<[f64; 3]>,
        #[arg(long, help = REPLACE)]
        index: Option<usize>,
    },
    #[command(about = "Move the particles in circles around a point, a control point or the pointer")]
    Orbit {
        #[arg(allow_hyphen_values = true, help = "X,Y in pixels of the layer, p0 to p7, or cursor")]
        center: String,
        #[arg(long, allow_negative_numbers = true, help = "Pixels a second; negative turns the other way")]
        strength: f64,
        #[arg(long, allow_negative_numbers = true, help = "Pixels beyond which it does not turn them")]
        radius: Option<f64>,
        #[arg(long, value_parser = FALLOFF, help = "How the pull weakens with distance")]
        falloff: Option<String>,
        #[arg(long, help = REPLACE)]
        index: Option<usize>,
    },
}

fn place(text: &str) -> Value {
    pair(text).map_or_else(|_| json!(text), |p| json!(p))
}

pub(crate) fn pull(what: Pull) -> (Option<usize>, Option<Value>) {
    match what {
        Pull::Noise { amount, index } => (index, Some(json!({ "noise": amount }))),
        Pull::Cap { speed, index } => (index, Some(json!({ "cap": speed }))),
        Pull::Torque { amount, index } => (index, Some(json!({ "torque": amount }))),
        Pull::SpinDrag { amount, index } => (index, Some(json!({ "spin_drag": amount }))),
        Pull::Vortex { center, strength, radius, falloff, inner, outer, ring, index } => {
            let mut v = json!({ "center": place(&center), "strength": strength });
            radius.iter().for_each(|r| v["radius"] = json!(r));
            falloff.iter().for_each(|f| v["falloff"] = json!(f));
            inner.iter().for_each(|i| v["inner"] = json!(i));
            outer.iter().for_each(|o| v["outer"] = json!(o));
            ring.iter().for_each(|[r, w, p]| v["ring"] = json!({ "radius": r, "width": w, "pull": p }));
            (index, Some(json!({ "vortex": v })))
        }
        Pull::Orbit { center, strength, radius, falloff, index } => {
            let mut o = json!({ "center": place(&center), "strength": strength });
            radius.iter().for_each(|r| o["radius"] = json!(r));
            falloff.iter().for_each(|f| o["falloff"] = json!(f));
            (index, Some(json!({ "orbit": o })))
        }
    }
}
