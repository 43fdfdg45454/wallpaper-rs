use crate::author::{DIR, ID, POINTER};
use crate::steps::value;
use clap::{ArgAction, Subcommand};
use std::path::PathBuf;
use wprs_editor::actions::Action;

#[derive(Subcommand, Debug)]
pub enum Layering {
    #[command(
        about = "Add a layer: image, video, text, shape, solid, spectrum, particles, sound, group or adjustment"
    )]
    AddLayer {
        #[arg(help = DIR)]
        dir: PathBuf,
        kind: String,
        #[arg(long, help = "Id of the new layer; made from the name, or else the kind, when omitted")]
        id: Option<String>,
        #[arg(
            long,
            help = "Name of the layer in your language: the base of its id and the words of a text layer"
        )]
        name: Option<String>,
        #[arg(long, help = "Asset of an image, video or sound layer, or system of a particles layer")]
        source: Option<String>,
        #[arg(long, help = "Group to add the layer to")]
        into: Option<String>,
        #[arg(long, help = "Position among its siblings, 0 at the bottom; on top when omitted")]
        at: Option<usize>,
    },
    #[command(about = "Remove a layer")]
    RemoveLayer {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        id: String,
    },
    #[command(about = "Copy a layer, with new ids, just above it")]
    DuplicateLayer {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        id: String,
    },
    #[command(about = "Move a layer to a position, 0 at the bottom, among the top layers or in a group")]
    MoveLayer {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        id: String,
        to: usize,
        #[arg(long, help = "Group to move the layer into")]
        into: Option<String>,
    },
    #[command(about = "Put layers of the same group into a new group")]
    Group {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(required = true)]
        ids: Vec<String>,
        #[arg(long, help = "Id of the new group")]
        id: Option<String>,
    },
    #[command(about = "Replace a group with its layers")]
    Ungroup {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        id: String,
    },
    #[command(about = "Hide a layer, or mute a sound layer; --hidden false shows it again")]
    Hide {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        id: String,
        #[arg(long, default_value_t = true, action = ArgAction::Set)]
        hidden: bool,
    },
    #[command(about = "Lock a layer against changes on the canvas; --locked false unlocks it")]
    Lock {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        id: String,
        #[arg(long, default_value_t = true, action = ArgAction::Set)]
        locked: bool,
    },
    #[command(about = "Show only this layer in previews, or every layer again without an id")]
    Solo {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        id: Option<String>,
    },
    #[command(
        about = "Add or replace a keyframe of an animatable value, remove it with --remove or move it with --from"
    )]
    Keyframe {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = POINTER)]
        pointer: String,
        #[arg(help = "Seconds on the timeline, or on the scene clock without one")]
        time: f64,
        #[arg(allow_hyphen_values = true, help = "JSON value; the current value when omitted")]
        value: Option<String>,
        #[arg(long, help = "Easing towards the next keyframe, such as in_out_sine, or [x1,y1,x2,y2]")]
        ease: Option<String>,
        #[arg(long, help = "Timeline that drives the keyframes")]
        timeline: Option<String>,
        #[arg(long)]
        remove: bool,
        #[arg(
            long,
            value_name = "TIME",
            help = "Move the keyframe at this time to TIME, keeping its value and easing"
        )]
        from: Option<f64>,
    },
}

pub(crate) fn action(command: Layering) -> (PathBuf, Action) {
    match command {
        Layering::AddLayer { dir, kind, id, name, source, into, at } => {
            (dir, Action::AddLayer { kind, id, name, source, into, at })
        }
        Layering::RemoveLayer { dir, id } => (dir, Action::RemoveLayer { id }),
        Layering::DuplicateLayer { dir, id } => (dir, Action::DuplicateLayer { id }),
        Layering::MoveLayer { dir, id, to, into } => (dir, Action::MoveLayer { id, to, into }),
        Layering::Group { dir, ids, id } => (dir, Action::Group { ids, id }),
        Layering::Ungroup { dir, id } => (dir, Action::Ungroup { id }),
        Layering::Hide { dir, id, hidden } => (dir, Action::Hide { id, hidden }),
        Layering::Lock { dir, id, locked } => (dir, Action::Lock { id, locked }),
        Layering::Solo { dir, id } => (dir, Action::Solo { id }),
        Layering::Keyframe { dir, pointer, time, value: v, ease, timeline, remove, from } => {
            let (value, ease) = (v.as_deref().map(value), ease.as_deref().map(value));
            (dir, Action::Keyframe { pointer, time, value, ease, timeline, remove, from })
        }
    }
}
