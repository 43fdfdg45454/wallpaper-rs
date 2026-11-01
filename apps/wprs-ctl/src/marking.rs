use crate::author::{DIR, ID};
use crate::rigging::pair;
use clap::Subcommand;
use serde_json::json;
use std::path::PathBuf;
use wprs_editor::actions::{Action, AttachEdit, PointEdit, Spot};

#[derive(Subcommand, Debug)]
pub enum Marking {
    #[command(about = "Pin a layer to a bone or a point of a mesh layer, or let it go")]
    Attach {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
        #[arg(long, value_name = "MESH_LAYER", help = "The mesh layer it follows")]
        to: Option<String>,
        #[arg(long, conflicts_with = "point", help = "The bone it follows")]
        bone: Option<String>,
        #[arg(
            long,
            value_name = "X,Y or NAME",
            help = "A new point at X,Y in fractions of the mesh layer, or a node of its file"
        )]
        point: Option<String>,
        #[arg(long, allow_hyphen_values = true, value_name = "X,Y", help = "Pixels along the bone")]
        offset: Option<String>,
        #[arg(
            long,
            allow_hyphen_values = true,
            value_name = "X,Y",
            help = "Move it by X,Y pixels of the box of the mesh layer, along the bone at its rest pose"
        )]
        nudge: Option<String>,
        #[arg(long, value_delimiter = ',', value_parser = ["position", "rotation", "scale"], help = "What it follows, comma separated (all when unset)")]
        inherit: Option<Vec<String>>,
        #[arg(long, conflicts_with_all = ["to", "bone", "point"], help = "Follow nothing again; a point placed for it alone goes too")]
        detach: bool,
    },
    #[command(
        about = "Add, move, rehang, rename or remove an attachment point of a mesh layer, which follows the bone it hangs from"
    )]
    Point {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
        #[arg(help = "Name of the point")]
        name: String,
        #[arg(
            long,
            value_name = "X,Y",
            help = "Where it goes, as fractions of the layer; a new point hangs from the bone that leads the mesh there"
        )]
        at: Option<String>,
        #[arg(long, conflicts_with = "root", help = "The bone it hangs from")]
        parent: Option<String>,
        #[arg(long, help = "Hang it from no bone")]
        root: bool,
        #[arg(
            long = "rename",
            value_name = "NAME",
            help = "A new name, also changed where layers and chains name the point"
        )]
        to: Option<String>,
        #[arg(long, help = "Remove the point, and what follows or reaches for it")]
        remove: bool,
    },
}

pub(crate) fn action(command: Marking) -> (PathBuf, Action) {
    match command {
        Marking::Attach { dir, layer, to, bone, point, offset, nudge, inherit, detach } => {
            let point = point.map(|p| pair(&p).map_or(Spot::Name(p), Spot::At));
            let inherit =
                inherit.map(|l| l.iter().filter_map(|f| serde_json::from_value(json!(f)).ok()).collect());
            let (offset, nudge) = (offset.as_deref().and_then(pair), nudge.as_deref().and_then(pair));
            (dir, Action::Attach(AttachEdit { layer, to, bone, point, offset, inherit, nudge, detach }))
        }
        Marking::Point { dir, layer, name, at, parent, root, to, remove } => {
            let parent = if root { Some(String::new()) } else { parent };
            let at = at.as_deref().and_then(pair);
            (dir, Action::Point(PointEdit { layer, name, at, parent, to, remove }))
        }
    }
}
