use crate::author::{DIR, ID};
use crate::told::{fill, told};
use clap::Subcommand;
use std::fs;
use std::path::PathBuf;
use wprs_editor::Action;
use wprs_editor::actions::PropertyEdit;

fn cells(text: &str) -> Result<[u32; 2], String> {
    let parsed =
        text.split_once(',').and_then(|(a, b)| Some([a.trim().parse().ok()?, b.trim().parse().ok()?]));
    parsed.ok_or_else(|| fill(told("%1 is not ROWS,COLUMNS"), &[&text]))
}

#[derive(Subcommand, Debug)]
pub enum Shaping {
    #[command(about = "Copy a picture, video, sound, font or .cube table into the folder as an asset")]
    Import {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = "The file to copy")]
        file: PathBuf,
        #[arg(long, help = "Asset id; made from the file name when omitted")]
        id: Option<String>,
        #[arg(long, help = "Read the picture as data, such as a normal map or a mask, not as colors")]
        linear: bool,
        #[arg(long, value_name = "ROWS,COLUMNS", value_parser = cells, help = "Split the picture into a sprite sheet of equal cells")]
        atlas: Option<[u32; 2]>,
    },
    #[command(
        about = "Add, change, order, mask or remove the effects of a layer with the checks of the scene editor"
    )]
    Effects {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
        #[command(subcommand)]
        what: crate::effecting::Effect,
    },
    #[command(
        about = "Place, turn, scale, size and blend a layer, or change the instance of a particles layer"
    )]
    Layer {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
        #[command(subcommand)]
        what: crate::staging::Stage,
    },
    #[command(about = "Set the origin, license, tags, authors and display of the package")]
    Manifest {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[command(subcommand)]
        what: crate::manifesting::Meta,
    },
    #[command(about = "Set the range, step, default value and place of a user property")]
    Property {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = "Id of a user property")]
        id: String,
        #[arg(long, allow_hyphen_values = true, help = "Smallest value of a slider")]
        min: Option<f64>,
        #[arg(long, allow_hyphen_values = true, help = "Largest value of a slider")]
        max: Option<f64>,
        #[arg(allow_negative_numbers = true, long, help = "Step of a slider")]
        step: Option<f64>,
        #[arg(long, allow_hyphen_values = true, help = "Default value as JSON or text")]
        default: Option<String>,
        #[arg(long, help = "Place among the properties of its group, 0 first")]
        order: Option<usize>,
    },
    #[command(about = "Let the scene glow or move in depth with the pointer and the tilt of a phone")]
    Canvas {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[command(subcommand)]
        what: crate::backdrop::Backdrop,
    },
}

pub(crate) fn action(command: Shaping) -> (PathBuf, Action) {
    match command {
        Shaping::Import { dir, file, id, linear, atlas } => {
            (dir, Action::Import { file: fs::canonicalize(&file).unwrap_or(file), id, linear, atlas })
        }
        Shaping::Effects { dir, layer, what } => (dir, Action::Effects(crate::effected::edit(layer, what))),
        Shaping::Layer { dir, layer, what } => (dir, Action::Layer(crate::staging::edit(layer, what))),
        Shaping::Manifest { dir, what } => (dir, Action::Manifest(crate::manifesting::edit(what))),
        Shaping::Property { dir, id, min, max, step, default, order } => {
            let default = default.as_deref().map(crate::steps::value);
            (dir, Action::Property(PropertyEdit { id, min, max, step, default, order }))
        }
        Shaping::Canvas { dir, what } => (dir, Action::Canvas(crate::backdrop::edit(what))),
    }
}
