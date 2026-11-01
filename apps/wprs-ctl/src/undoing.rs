use crate::author::DIR;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum Undoing {
    #[command(about = "Undo the last change")]
    Undo {
        #[arg(help = DIR)]
        dir: PathBuf,
    },
    #[command(about = "Redo the last undone change")]
    Redo {
        #[arg(help = DIR)]
        dir: PathBuf,
    },
}
