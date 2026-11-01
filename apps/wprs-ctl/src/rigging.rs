use crate::author::{Author, DIR, ID};
use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;
use wprs_editor::Project;
use wprs_editor::actions::Action;

#[derive(Subcommand, Debug)]
pub enum Rigging {
    #[command(
        about = "Cut a mesh layer out of a picture of the scene, along a grid of cells or along its outline, ready to take bones"
    )]
    MeshFromPicture {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = "Id of the picture asset")]
        image: String,
        #[arg(long, help = "Follow the outline of the opaque part instead of a full grid")]
        outline: bool,
        #[arg(long, value_parser = clap::value_parser!(u32).range(2..=64), help = "Cells along the longer side, from 2 to 64 (16 when omitted)")]
        cells: Option<u32>,
        #[arg(long, help = "Id of the new asset and layer")]
        id: Option<String>,
    },
    #[command(
        about = "Add, move, rehang or remove a bone of a mesh layer cut from a picture; the weights of its vertices follow the bones"
    )]
    Bone {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
        #[arg(help = "Name of the bone")]
        name: String,
        #[arg(long, value_name = "X,Y", help = "Where its head goes, as fractions of the layer")]
        at: Option<String>,
        #[arg(long, help = "The bone it hangs from")]
        parent: Option<String>,
        #[arg(long, help = "Remove the bone; its children hang from its parent")]
        remove: bool,
    },
    #[command(about = "Print the bones, parts, blend shapes, animations and clips of a mesh layer as JSON")]
    Bones {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
    },
}

pub(crate) fn action(command: Rigging) -> Result<(PathBuf, Action), Box<Author>> {
    Ok(match command {
        Rigging::MeshFromPicture { dir, image, outline, cells, id } => {
            (dir, Action::MeshFromPicture { image, outline, cells, id })
        }
        Rigging::Bone { dir, layer, name, at, parent, remove } => {
            let pair = |p: &str| {
                p.split_once(',').and_then(|(x, y)| Some([x.trim().parse().ok()?, y.trim().parse().ok()?]))
            };
            let at = at.as_deref().and_then(pair);
            (dir, Action::Bone { layer, name, parent, at, remove })
        }
        other => return Err(Box::new(Author::Rigging(other))),
    })
}

pub(crate) fn run(command: Rigging) -> Result<()> {
    if let Rigging::Bones { dir, layer } = command {
        let project = Project::open(&dir)?;
        let found = wprs_editor::rigs::rig(&project.dir, &project.doc, &layer);
        anyhow::ensure!(!found.is_null(), "there is no layer {layer:?}");
        println!("{}", serde_json::to_string_pretty(&found)?);
    }
    Ok(())
}
