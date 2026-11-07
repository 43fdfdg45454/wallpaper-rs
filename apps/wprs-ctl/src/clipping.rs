use crate::author::{Author, DIR, ID};
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use std::path::PathBuf;
use wprs_editor::actions::{Action, ClipEdit, ExpressionEdit};

#[derive(Subcommand, Debug)]
pub enum Clipping {
    #[command(
        about = "Add, change, rename or remove a clip of a mesh or model layer; --play also adds an animation layer that plays a new clip, and removing it also removes the animation layers that play it"
    )]
    Clip {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
        #[arg(help = "Id of the clip")]
        id: String,
        #[arg(long, help = "Add it, of 2 s unless --length says otherwise")]
        add: bool,
        #[arg(long, requires = "add", help = "Also add an animation layer that plays the new clip")]
        play: bool,
        #[arg(long, conflicts_with = "add", help = "Remove it")]
        remove: bool,
        #[arg(long = "rename", value_name = "ID", help = "A new id")]
        to: Option<String>,
        #[arg(long, help = "Length in seconds")]
        length: Option<f32>,
        #[arg(id = "loop", long = "loop", value_parser = ["repeat", "pingpong", "once"], help = "How it plays")]
        repeat: Option<String>,
        #[arg(long, help = "Play this animation of the file, whole")]
        animation: Option<String>,
        #[arg(
            long,
            value_name = "SECONDS",
            help = "Start the clip later in its animation, keeping its length"
        )]
        from: Option<f32>,
    },
    #[command(
        about = "Set, move, copy or remove a key of a clip of a mesh or model layer for a bone, a blend shape or the order of a part; a bone keyed without values keeps the pose it has there; --ease takes any easing of the scene format or [x1,y1,x2,y2], --in-handle and --out-handle bend one channel with Bézier handles"
    )]
    Key(crate::clipkey::KeyArgs),
    #[command(about = "Print the keys of a clip of a mesh or model layer, row by row, as JSON")]
    Keys {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
        #[arg(help = "Id of the clip")]
        clip: String,
    },
    #[command(
        about = "Set, rename or remove an expression of a mesh layer, a named set of blend shape weights"
    )]
    Expression {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
        #[arg(help = "Name of the expression")]
        name: String,
        #[arg(
            long = "shape",
            value_name = "SHAPE=WEIGHT",
            help = "A blend shape and its weight from -1 to 1"
        )]
        shapes: Vec<String>,
        #[arg(long = "rename", value_name = "NAME", help = "A new name")]
        to: Option<String>,
        #[arg(long, help = "Remove the expression")]
        remove: bool,
    },
    #[command(flatten)]
    Limbing(crate::limbing::Limbing),
}

pub(crate) fn action(command: Clipping) -> Result<(PathBuf, Action), Box<Author>> {
    Ok(match command {
        Clipping::Clip { dir, layer, id, add, play, remove, to, length, repeat, animation, from } => {
            let repeat = repeat.and_then(|r| serde_json::from_value(json!(r)).ok());
            let clip = ClipEdit { layer, id, add, remove, to, length, repeat, animation, from, play };
            (dir, Action::Clip(clip))
        }
        Clipping::Key(args) => args.action(),
        Clipping::Expression { dir, layer, name, shapes, to, remove } => {
            let parsed = shapes.iter().map(|s| {
                s.split_once('=').and_then(|(k, w)| Some((k.trim().to_owned(), w.trim().parse().ok()?)))
            });
            let shapes = parsed.collect::<Option<_>>().unwrap_or_default();
            (dir, Action::Expression(ExpressionEdit { layer, name, shapes, remove, to }))
        }
        Clipping::Limbing(command) => return crate::limbing::action(command),
        other => return Err(Box::new(Author::Clipping(other))),
    })
}

pub(crate) fn run(command: Clipping) -> Result<()> {
    if let Clipping::Keys { dir, layer, clip } = command {
        crate::rigging::print(&dir, &layer, |p| wprs_editor::rigs::keys(&p.dir, &p.doc, &layer, &clip))?;
    }
    Ok(())
}
