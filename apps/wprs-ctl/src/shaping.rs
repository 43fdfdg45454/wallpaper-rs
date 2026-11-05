use crate::author::{DIR, ID};
use clap::Subcommand;
use std::fs;
use std::path::PathBuf;
use wprs_editor::Action;

#[derive(Subcommand, Debug)]
pub enum Shaping {
    #[command(about = "Copy a picture, video, sound, font or .cube table into the folder as an asset")]
    Import {
        #[arg(help = DIR)]
        dir: PathBuf,
        file: PathBuf,
        #[arg(long, help = "Asset id; made from the file name when omitted")]
        id: Option<String>,
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
}

pub(crate) fn action(command: Shaping) -> (PathBuf, Action) {
    match command {
        Shaping::Import { dir, file, id } => {
            (dir, Action::Import { file: fs::canonicalize(&file).unwrap_or(file), id })
        }
        Shaping::Effects { dir, layer, what } => (dir, Action::Effects(crate::effected::edit(layer, what))),
    }
}
