use crate::rigging::pair;
use clap::{Args, Subcommand};
use serde_json::json;
use wprs_editor::actions::BodyEdit;

#[derive(Args, Debug, Default)]
pub struct Tuning {
    #[arg(long, value_parser = ["spring", "rigid", "rope"], help = "spring (the default), rigid or rope")]
    mode: Option<String>,
    #[arg(long, help = "How hard it pulls back to its pose, from 0 to 1")]
    stiffness: Option<f32>,
    #[arg(long, help = "How fast it calms down, from 0 to 1")]
    damping: Option<f32>,
    #[arg(long, help = "How much it stays behind when its parent moves, from 0 to 1")]
    inertia: Option<f32>,
    #[arg(long, allow_hyphen_values = true, help = "How much it is pulled down, from -10 to 10")]
    gravity: Option<f32>,
    #[arg(long, help = "How much the wind pushes it, from 0 to 1")]
    wind: Option<f32>,
    #[arg(
        long,
        allow_hyphen_values = true,
        value_name = "MIN,MAX",
        help = "Degrees each link may turn from its pose"
    )]
    rotation: Option<String>,
    #[arg(
        long,
        allow_hyphen_values = true,
        value_name = "MIN,MAX",
        help = "Pixels the head of the bone may move each way"
    )]
    translation: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Swing {
    #[command(about = "Make bones swing, or change how they swing")]
    Add {
        #[arg(required = true, value_name = "BONE")]
        bones: Vec<String>,
        #[command(flatten)]
        tune: Tuning,
    },
    #[command(
        about = "Draw a rope of new bones NAME_1, NAME_2... along X,Y;X,Y;... in fractions of the layer, swinging as a rope"
    )]
    Rope {
        #[arg(value_name = "NAME")]
        bones: String,
        #[arg(value_name = "X,Y;X,Y;...")]
        rope: String,
        #[arg(long, help = "The bone the rope hangs from")]
        parent: Option<String>,
        #[command(flatten)]
        tune: Tuning,
    },
    #[command(about = "Stop bones swinging")]
    Remove {
        #[arg(required = true, value_name = "BONE")]
        bones: Vec<String>,
    },
}

fn tuned(layer: String, bones: Vec<String>, t: Tuning) -> BodyEdit {
    let mode = t.mode.and_then(|m| serde_json::from_value(json!(m)).ok());
    let (rotation, translation) =
        (t.rotation.as_deref().and_then(pair), t.translation.as_deref().and_then(pair));
    let (stiffness, damping, inertia, gravity, wind) = (t.stiffness, t.damping, t.inertia, t.gravity, t.wind);
    let (rope, parent, remove) = (Vec::new(), None, false);
    BodyEdit {
        layer,
        bones,
        mode,
        stiffness,
        damping,
        inertia,
        gravity,
        wind,
        rotation,
        translation,
        rope,
        parent,
        remove,
    }
}

pub(crate) fn edit(layer: String, change: Swing) -> BodyEdit {
    match change {
        Swing::Add { bones, tune } => tuned(layer, bones, tune),
        Swing::Rope { bones, rope, parent, tune } => {
            let points = rope.split(';').filter(|p| !p.trim().is_empty()).map(pair).collect::<Option<_>>();
            BodyEdit { rope: points.unwrap_or_default(), parent, ..tuned(layer, vec![bones], tune) }
        }
        Swing::Remove { bones } => BodyEdit { remove: true, ..tuned(layer, bones, Tuning::default()) },
    }
}
