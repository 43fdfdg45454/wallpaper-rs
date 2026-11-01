use crate::author::{Author, DIR, ID};
use crate::rigging::pair;
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use std::path::PathBuf;
use wprs_editor::actions::{Action, ClipEdit, ClipKey, ExpressionEdit};

#[derive(Subcommand, Debug)]
pub enum Clipping {
    #[command(
        about = "Add, change, rename or remove a clip of a mesh layer; removing it also removes the animation layers that play it"
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
        #[arg(long, conflicts_with = "add", help = "Remove it")]
        remove: bool,
        #[arg(long = "rename", value_name = "ID", help = "A new id")]
        to: Option<String>,
        #[arg(long, help = "Length in seconds")]
        length: Option<f32>,
        #[arg(id = "loop", long = "loop", value_parser = ["repeat", "pingpong", "once"], help = "How it plays")]
        repeat: Option<String>,
    },
    #[command(
        about = "Set, move, copy or remove a key of a clip of a mesh layer for a bone, a blend shape or the order of a part; a bone keyed without values keeps the pose it has there"
    )]
    Key {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
        #[arg(help = "Id of the clip")]
        clip: String,
        #[arg(help = "Seconds from the start of the clip")]
        time: f64,
        #[arg(long, conflicts_with_all = ["shape", "order"], help = "The bone keyed")]
        bone: Option<String>,
        #[arg(long, allow_hyphen_values = true, help = "Degrees clockwise added to its rest pose")]
        rotation: Option<f32>,
        #[arg(
            long,
            allow_hyphen_values = true,
            value_name = "X,Y",
            help = "Pixels, y down, in the space of its parent"
        )]
        position: Option<String>,
        #[arg(long, allow_hyphen_values = true, value_name = "S or X,Y", help = "Scale of its rest pose")]
        scale: Option<String>,
        #[arg(long, conflicts_with = "order", help = "The blend shape keyed")]
        shape: Option<String>,
        #[arg(long, allow_hyphen_values = true, help = "Weight of the blend shape, from -1 to 1")]
        weight: Option<f32>,
        #[arg(long, help = "The part whose order is keyed")]
        order: Option<String>,
        #[arg(long, allow_hyphen_values = true, help = "Order of the part, from -1000 to 1000")]
        value: Option<f32>,
        #[arg(long, value_parser = ["step", "linear", "cubic"], help = "How the key moves on to the next one")]
        ease: Option<String>,
        #[arg(long, help = "Remove the key")]
        remove: bool,
        #[arg(long, value_name = "SECONDS", help = "Move the key that is at these seconds")]
        from: Option<f64>,
        #[arg(long, requires = "from", help = "Copy the key instead of moving it")]
        copy: bool,
    },
    #[command(about = "Print the keys of a clip of a mesh layer, row by row, as JSON")]
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
        Clipping::Clip { dir, layer, id, add, remove, to, length, repeat } => {
            let repeat = repeat.and_then(|r| serde_json::from_value(json!(r)).ok());
            (dir, Action::Clip(ClipEdit { layer, id, add, remove, to, length, repeat }))
        }
        Clipping::Key {
            dir,
            layer,
            clip,
            time,
            bone,
            rotation,
            position,
            scale,
            shape,
            weight,
            order,
            value,
            ease,
            remove,
            from,
            copy,
        } => {
            let position = position.as_deref().and_then(pair);
            let scale =
                scale.as_deref().and_then(|s| pair(s).or_else(|| s.trim().parse().ok().map(|v| [v, v])));
            let ease = ease.and_then(|e| serde_json::from_value(json!(e)).ok());
            (
                dir,
                Action::Key(ClipKey {
                    layer,
                    clip,
                    time,
                    bone,
                    rotation,
                    position,
                    scale,
                    shape,
                    weight,
                    order,
                    value,
                    ease,
                    remove,
                    from,
                    copy,
                }),
            )
        }
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
