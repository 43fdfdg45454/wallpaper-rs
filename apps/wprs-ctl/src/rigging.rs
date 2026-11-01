use crate::author::{Author, DIR, ID};
use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;
use wprs_editor::Project;
use wprs_editor::actions::{Action, AutoWeights, BoneEdit, DepthStroke, MeshCut, WeightStroke};
use wprs_editor::skin::Mode;

const STROKE: &str = "The stroke as fractions of the layer, X,Y;X,Y;...";

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
        about = "Add, move, rehang, rename or remove a bone of a mesh layer; painted weights stay, automatic ones follow the bones"
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
        #[arg(long, conflicts_with = "root", help = "The bone it hangs from")]
        parent: Option<String>,
        #[arg(long, help = "Hang it from no other bone")]
        root: bool,
        #[arg(
            long = "rename",
            value_name = "NAME",
            help = "A new name, also changed where the scene names the bone"
        )]
        to: Option<String>,
        #[arg(long, help = "Remove the bone; its children hang from its parent, which takes its weights")]
        remove: bool,
    },
    #[command(
        about = "Raise or lower the depth of the vertices of a mesh layer along a stroke, for the perspective of its depth and the order of what covers what"
    )]
    MeshDepth {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
        #[arg(help = STROKE)]
        points: String,
        #[arg(
            long,
            allow_hyphen_values = true,
            help = "Pixels towards the viewer at the middle of the brush, from -1000 to 1000; below zero lowers"
        )]
        by: f32,
        #[arg(long, default_value_t = 0.1, help = "Radius of the brush, a fraction of the longer side")]
        radius: f32,
    },
    #[command(
        about = "Paint the weights of one bone on the vertices of a mesh layer along a stroke; at most 4 bones per vertex, adding up to 1"
    )]
    Weights {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
        #[arg(help = "The bone painted")]
        bone: String,
        #[arg(help = STROKE)]
        points: String,
        #[arg(
            long,
            value_parser = ["add", "subtract", "smooth", "normalise"],
            help = "add, subtract, smooth (towards the neighbours) or normalise (drops weights under --by)"
        )]
        mode: String,
        #[arg(long, help = "Strength at the middle of the brush, from 0 to 1")]
        by: f32,
        #[arg(long, default_value_t = 0.1, help = "Radius of the brush, a fraction of the longer side")]
        radius: f32,
        #[arg(
            long,
            default_value_t = 0.5,
            help = "How softly the brush fades towards its edge, from 0 to 1"
        )]
        falloff: f32,
        #[arg(long = "lock", value_name = "BONE", help = "A bone whose weights the stroke keeps")]
        lock: Vec<String>,
    },
    #[command(
        about = "Give every vertex of a mesh layer weights from the nearest bones again, replacing painted ones"
    )]
    AutoWeights {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
    },
    #[command(
        about = "Print the bones, parts, blend shapes, expressions, animations and clips of a mesh layer as JSON"
    )]
    Bones {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
    },
    #[command(about = "Print each vertex of a mesh layer with its place, part and bone weights as JSON")]
    VertexWeights {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
    },
}

pub(crate) fn pair(p: &str) -> Option<[f32; 2]> {
    p.split_once(',').and_then(|(x, y)| Some([x.trim().parse().ok()?, y.trim().parse().ok()?]))
}

fn stroke(points: &str) -> Vec<[f32; 2]> {
    let points = points.split(';').filter(|p| !p.trim().is_empty()).map(pair).collect::<Option<_>>();
    points.unwrap_or_default()
}

pub(crate) fn action(command: Rigging) -> Result<(PathBuf, Action), Box<Author>> {
    Ok(match command {
        Rigging::MeshFromPicture { dir, image, outline, cells, id } => {
            (dir, Action::MeshFromPicture(MeshCut { image, outline, cells, id }))
        }
        Rigging::Bone { dir, layer, name, at, parent, root, to, remove } => {
            let parent = if root { Some(String::new()) } else { parent };
            let at = at.as_deref().and_then(pair);
            (dir, Action::Bone(BoneEdit { layer, name, parent, at, remove, to }))
        }
        Rigging::MeshDepth { dir, layer, points, by, radius } => {
            (dir, Action::MeshDepth(DepthStroke { layer, points: stroke(&points), radius, by }))
        }
        Rigging::Weights { dir, layer, bone, points, mode, by, radius, falloff, lock } => {
            let mode: Mode = serde_json::from_value(serde_json::json!(mode)).unwrap_or(Mode::Add);
            (
                dir,
                Action::Weights(WeightStroke {
                    layer,
                    bone,
                    points: stroke(&points),
                    mode,
                    by,
                    radius,
                    falloff,
                    lock,
                }),
            )
        }
        Rigging::AutoWeights { dir, layer } => (dir, Action::AutoWeights(AutoWeights { layer })),
        other => return Err(Box::new(Author::Rigging(other))),
    })
}

pub(crate) fn print(
    dir: &std::path::Path,
    layer: &str,
    read: impl Fn(&Project) -> serde_json::Value,
) -> Result<()> {
    let project = Project::open(dir)?;
    let found = read(&project);
    anyhow::ensure!(!found.is_null(), "there is no mesh layer {layer:?} with that");
    println!("{}", serde_json::to_string_pretty(&found)?);
    Ok(())
}

pub(crate) fn run(command: Rigging) -> Result<()> {
    match command {
        Rigging::Bones { dir, layer } => {
            print(&dir, &layer, |p| wprs_editor::rigs::rig(&p.dir, &p.doc, &layer))
        }
        Rigging::VertexWeights { dir, layer } => {
            print(&dir, &layer, |p| wprs_editor::rigs::vertices(&p.dir, &p.doc, &layer))
        }
        _ => Ok(()),
    }
}
