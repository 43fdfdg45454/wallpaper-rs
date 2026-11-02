use crate::author::DIR;
use clap::{ArgAction, Subcommand};
use std::path::PathBuf;

pub use crate::posing::{GroupCmd, ModelCmd};

#[derive(Subcommand, Debug)]
pub enum Modeling {
    #[command(about = "Add, place, remove or describe a 3D model layer")]
    Model {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[command(subcommand)]
        what: ModelCmd,
    },
    #[command(
        about = "Make 3D groups, turn groups 3D or flat, let flat layers in them test depth, or show their handles"
    )]
    Group3d {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[command(subcommand)]
        what: GroupCmd,
    },
    #[command(about = "Change a material of a model layer by its name, or list the materials")]
    Material {
        #[arg(help = DIR)]
        dir: PathBuf,
        layer: String,
        #[arg(help = "Name of the material in the model; leave out with --list")]
        name: Option<String>,
        #[arg(long, help = "pbr, unlit or the id of a package effect")]
        shader: Option<String>,
        #[arg(long, action = ArgAction::Set)]
        lit: Option<bool>,
        #[arg(long, action = ArgAction::Set)]
        double_sided: Option<bool>,
        #[arg(long, action = ArgAction::Set)]
        cast_shadows: Option<bool>,
        #[arg(long, action = ArgAction::Set)]
        receive_shadows: Option<bool>,
        #[arg(long, help = "Color such as #ff8800, or none")]
        color: Option<String>,
        #[arg(long, help = "Glow color such as #ff8800, or none")]
        emissive: Option<String>,
        #[arg(
            long = "param",
            value_name = "ID=VALUE",
            help = "metallic, roughness, normal_scale or occlusion"
        )]
        params: Vec<String>,
        #[arg(long, help = "Drop the changes of this material")]
        clear: bool,
        #[arg(long, help = "List the materials with their maps instead")]
        list: bool,
    },
    #[command(about = "Set the 3D camera of the scene, or copy it from the authoring view")]
    Camera3d {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(long, help = "Field of view in degrees, 1 to 179")]
        fov: Option<f64>,
        #[arg(long)]
        near: Option<f64>,
        #[arg(long)]
        far: Option<f64>,
        #[arg(long, value_name = "X,Y,Z")]
        eye: Option<String>,
        #[arg(long, value_name = "X,Y,Z")]
        center: Option<String>,
        #[arg(long, value_name = "X,Y,Z")]
        up: Option<String>,
        #[arg(long, allow_hyphen_values = true, help = "Turn the camera around its view, in degrees")]
        roll: Option<f64>,
        #[arg(long, help = "Take the place of the authoring view")]
        from_view: bool,
        #[arg(long, help = "Remove the camera")]
        clear: bool,
    },
    #[command(about = "Move the authoring view of the 3D scene, apart from the scene camera")]
    View3d {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(long, value_name = "DX,DY", allow_hyphen_values = true, help = "Degrees to turn around")]
        orbit: Option<String>,
        #[arg(long, value_name = "DX,DY", allow_hyphen_values = true, help = "Pixels to slide")]
        pan: Option<String>,
        #[arg(long, help = "Factor to come closer")]
        zoom: Option<f64>,
        #[arg(long, help = "Look at a model layer")]
        frame: Option<String>,
        #[arg(long, action = ArgAction::Set, help = "Look through the scene camera")]
        through: Option<bool>,
        #[arg(long, help = "Go back to the scene camera")]
        reset: bool,
    },
    #[command(about = "Keep colors brighter than white and pick how their tones are mapped")]
    Tonemap {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(long, action = ArgAction::Set)]
        hdr: Option<bool>,
        #[arg(long = "map", value_parser = ["none", "aces", "agx"])]
        map: Option<String>,
        #[arg(long, help = "Show the left part of the preview without the tone map, 0 to 1")]
        split: Option<f64>,
    },
    #[command(
        about = "Describe a binary glTF file: triangles, materials, textures, bones, clips and what each tier gets"
    )]
    ModelInfo { file: PathBuf },
    #[command(
        about = "Apply one 3D edit given as JSON, as the app sends it; the commands above write the same"
    )]
    Scene3d {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(long, value_name = "JSON")]
        model: Option<String>,
        #[arg(long, value_name = "JSON")]
        group: Option<String>,
        #[arg(long, value_name = "JSON")]
        space: Option<String>,
        #[arg(long, value_name = "JSON")]
        depth_test: Option<String>,
        #[arg(long, value_name = "JSON")]
        material: Option<String>,
        #[arg(long, value_name = "JSON")]
        camera: Option<String>,
        #[arg(long, value_name = "JSON")]
        view: Option<String>,
        #[arg(long, value_name = "JSON")]
        tones: Option<String>,
    },
    #[command(about = "Show what the models cost at each quality level and on each device tier")]
    Cost3d {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(long, value_parser = ["q0", "q1", "q2", "q3"])]
        quality: Option<String>,
    },
}
