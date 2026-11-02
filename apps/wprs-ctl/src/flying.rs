use crate::author::{DIR, ID};
use clap::{ArgAction, Subcommand};
use std::path::PathBuf;

pub use crate::flights::{AnimCmd, PathCmd};

#[derive(Subcommand, Debug)]
pub enum Flying {
    #[command(
        about = "Animate a mesh or model layer: animation layers, root motion, physics presets, blend shapes, and list its clips and bones"
    )]
    Animate {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
        #[command(subcommand)]
        what: Box<AnimCmd>,
    },
    #[command(
        about = "Let a point, spot or directional light cast shadows, size and bias its map, or show them per tier"
    )]
    Shadows {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = ID)]
        layer: String,
        #[arg(long, action = ArgAction::Set)]
        on: Option<bool>,
        #[arg(long, help = "Pixels a side of its map, 256 to 4096")]
        resolution: Option<u32>,
        #[arg(long, help = "Pick the size of the map for each light kind and tier")]
        automatic: bool,
        #[arg(long, help = "How far surfaces lift away from their own shadow, 0 to 1")]
        bias: Option<f64>,
        #[arg(long, help = "Print the shadowed lights, their maps per tier and their frusta")]
        show: bool,
    },
    #[command(about = "Add, key, change, order, fade or show the paths the 3D camera flies along")]
    CameraPath {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[command(subcommand)]
        what: PathCmd,
    },
    #[command(
        about = "Start a material shader of the package from a template, or check it and print its errors"
    )]
    NewMaterial {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = "Id of the shader")]
        name: String,
        #[arg(long, help = "Only check the shader")]
        check: bool,
    },
    #[command(about = "Show the foliage mask, the chroma matte or the shadows in the 3D preview, or nothing")]
    Debug3d {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(value_parser = ["mask", "matte", "shadows", "none"])]
        show: String,
    },
}
