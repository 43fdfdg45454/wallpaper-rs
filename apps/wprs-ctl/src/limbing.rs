use crate::author::{Author, DIR, ID};
use crate::rigging::pair;
use clap::{Args, Subcommand};
use serde_json::json;
use std::path::PathBuf;
use wprs_editor::actions::{Action, IkEdit, PartsEdit, WindEdit};

#[derive(Subcommand, Debug)]
pub enum Limbing {
    #[command(about = "Add, change or remove a chain of bones of a mesh layer that reaches for a target")]
    Ik {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
        #[command(subcommand)]
        change: Chain,
    },
    #[command(about = "Make bones of a mesh layer swing, draw a rope of new bones, or stop them swinging")]
    Physics {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
        #[command(subcommand)]
        change: crate::swinging::Swing,
    },
    #[command(
        about = "Set the wind of the scene that swings bones, by its angle, strength and gusts, or stop it"
    )]
    Wind {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(
            long,
            allow_hyphen_values = true,
            help = "Degrees clockwise from the right, where it blows to"
        )]
        angle: Option<f32>,
        #[arg(long, help = "Pixels a second, from 0 to 10000")]
        strength: Option<f32>,
        #[arg(
            long,
            visible_alias = "noise",
            help = "How much it comes in gusts, from 0 to 1 (0.35 when unset)"
        )]
        gusts: Option<f32>,
        #[arg(long, conflicts_with_all = ["angle", "strength", "gusts"], help = "No wind")]
        off: bool,
    },
    #[command(flatten)]
    Marking(crate::marking::Marking),
    #[command(about = "Order, hide, show and cut the parts of a mesh layer to show only over another part")]
    Parts {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
        #[arg(long, num_args = 2, value_names = ["PART", "N"], allow_hyphen_values = true, help = "The order of a part, from -1000 to 1000")]
        order: Vec<String>,
        #[arg(long, value_name = "PART", help = "Hide a part")]
        hide: Vec<String>,
        #[arg(long, value_name = "PART", help = "Show a part again")]
        show: Vec<String>,
        #[arg(long, num_args = 2, value_names = ["PART", "TO"], help = "Show a part only over another one")]
        clip: Vec<String>,
        #[arg(long, value_name = "PART", help = "Show a part over everything again")]
        unclip: Vec<String>,
    },
}

#[derive(Args, Debug, Default)]
pub struct Reaching {
    #[arg(
        long,
        value_name = "cursor|bone:NAME|layer:ID|X,Y",
        help = "What it reaches for: the pointer, a bone or node, a layer, or a point in pixels of the layer from its top left"
    )]
    target: Option<String>,
    #[arg(id = "limits", long = "limit", num_args = 3, value_names = ["BONE", "MIN", "MAX"], allow_hyphen_values = true, help = "Degrees a bone of the chain may turn from its rest")]
    limits: Vec<String>,
    #[arg(long, value_name = "BONE", help = "Let a bone turn freely again")]
    unlimit: Vec<String>,
    #[arg(long, value_parser = ["clockwise", "counterclockwise"], help = "Which way a chain of two bones bends")]
    bend: Option<String>,
    #[arg(long, help = "How much it moves the pose, from 0 to 1")]
    weight: Option<f32>,
}

#[derive(Subcommand, Debug)]
pub enum Chain {
    #[command(about = "A new chain from ROOT down to TIP")]
    Add {
        root: String,
        tip: String,
        #[command(flatten)]
        reach: Reaching,
    },
    #[command(about = "Change chain N, 0 first")]
    Set {
        index: usize,
        #[arg(long, requires = "tip")]
        root: Option<String>,
        #[arg(long, requires = "root")]
        tip: Option<String>,
        #[command(flatten)]
        reach: Reaching,
    },
    #[command(about = "Remove chain N, 0 first")]
    Remove { index: usize },
}

fn target(t: &str) -> serde_json::Value {
    pair(t).map_or_else(|| json!(t), |p| json!(p))
}

fn chain(layer: String, index: Option<usize>, ends: (Option<String>, Option<String>), r: Reaching) -> IkEdit {
    let limits = r
        .limits
        .chunks(3)
        .filter_map(|c| Some((c[0].clone(), [c.get(1)?.parse().ok()?, c.get(2)?.parse().ok()?])));
    let bend = r.bend.and_then(|b| serde_json::from_value(json!(b)).ok());
    let (root, tip, target, weight) = (ends.0, ends.1, r.target.as_deref().map(target), r.weight);
    IkEdit {
        layer,
        index,
        root,
        tip,
        target,
        limits: limits.collect(),
        unlimit: r.unlimit,
        bend,
        weight,
        remove: false,
    }
}

pub(crate) fn action(command: Limbing) -> Result<(PathBuf, Action), Box<Author>> {
    Ok(match command {
        Limbing::Ik { dir, layer, change } => {
            let edit = match change {
                Chain::Add { root, tip, reach } => chain(layer, None, (Some(root), Some(tip)), reach),
                Chain::Set { index, root, tip, reach } => chain(layer, Some(index), (root, tip), reach),
                Chain::Remove { index } => {
                    IkEdit { remove: true, ..chain(layer, Some(index), (None, None), Reaching::default()) }
                }
            };
            (dir, Action::Ik(edit))
        }
        Limbing::Physics { dir, layer, change } => {
            (dir, Action::Physics(crate::swinging::edit(layer, change)))
        }
        Limbing::Wind { dir, angle, strength, gusts, off } => {
            (dir, Action::Wind(WindEdit { angle, strength, gusts, off }))
        }
        Limbing::Marking(command) => crate::marking::action(command),
        Limbing::Parts { dir, layer, order, hide, show, clip, unclip } => {
            let order =
                order.chunks(2).filter_map(|c| Some((c[0].clone(), c.get(1)?.parse().ok()?))).collect();
            let clip = clip.chunks(2).filter_map(|c| Some((c[0].clone(), c.get(1)?.clone()))).collect();
            (dir, Action::Parts(PartsEdit { layer, order, hide, show, clip, unclip }))
        }
    })
}
