use crate::author::DIR;
use clap::{ArgAction, Subcommand};
use std::path::PathBuf;

pub use crate::posing::{GroupCmd, ModelCmd};

const EDIT: &str = "One edit as JSON, as the app sends it (see docs/specs/editor.md)";

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
        #[arg(help = "The model layer")]
        layer: String,
        #[arg(help = "Name of the material in the model; leave out with --list")]
        name: Option<String>,
        #[arg(long, help = "pbr, unlit, fur, foliage, chroma or the id of a package material shader")]
        shader: Option<String>,
        #[arg(help = "true to light the material, false to show its colors flat", long, action = ArgAction::Set)]
        lit: Option<bool>,
        #[arg(help = "true to draw both faces of each triangle", long, action = ArgAction::Set)]
        double_sided: Option<bool>,
        #[arg(help = "true to let the material cast shadows", long, action = ArgAction::Set)]
        cast_shadows: Option<bool>,
        #[arg(help = "true to let shadows fall on the material", long, action = ArgAction::Set)]
        receive_shadows: Option<bool>,
        #[arg(long, help = "Color such as #ff8800, or none")]
        color: Option<String>,
        #[arg(long, help = "Glow color such as #ff8800, or none")]
        emissive: Option<String>,
        #[arg(
            long = "param",
            value_name = "ID=VALUE",
            help = "metallic, roughness, normal_scale, occlusion, or layers, length, density, comb_x, comb_y, comb_z of fur, strength and speed of foliage, threshold and softness of chroma"
        )]
        params: Vec<String>,
        #[arg(long, help = "Key color of a chroma material such as #00ff00")]
        key: Option<String>,
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
        #[arg(help = "Nearest distance the camera draws", long)]
        near: Option<f64>,
        #[arg(help = "Farthest distance the camera draws", long)]
        far: Option<f64>,
        #[arg(help = "Where the camera stands", long, value_name = "X,Y,Z")]
        eye: Option<String>,
        #[arg(help = "The point the camera looks at", long, value_name = "X,Y,Z")]
        center: Option<String>,
        #[arg(help = "Which way is up for the camera", long, value_name = "X,Y,Z")]
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
        #[arg(help = "true to keep colors brighter than white", long, action = ArgAction::Set)]
        hdr: Option<bool>,
        #[arg(help = "How bright tones are brought into the screen: none, aces or agx", long = "map", value_parser = ["none", "aces", "agx"])]
        map: Option<String>,
        #[arg(long, help = "Show the left part of the preview without the tone map, 0 to 1")]
        split: Option<f64>,
    },
    #[command(
        about = "Describe a binary glTF file: triangles, materials, textures, bones, clips and what each tier gets"
    )]
    ModelInfo {
        #[arg(help = "A binary glTF file (.glb)")]
        file: PathBuf,
    },
    #[command(
        about = "Apply one 3D edit given as JSON, as the app sends it; the commands above write the same"
    )]
    Scene3d {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = EDIT, long, value_name = "JSON")]
        model: Option<String>,
        #[arg(help = EDIT, long, value_name = "JSON")]
        group: Option<String>,
        #[arg(help = EDIT, long, value_name = "JSON")]
        space: Option<String>,
        #[arg(help = EDIT, long, value_name = "JSON")]
        depth_test: Option<String>,
        #[arg(help = EDIT, long, value_name = "JSON")]
        material: Option<String>,
        #[arg(help = EDIT, long, value_name = "JSON")]
        camera: Option<String>,
        #[arg(help = EDIT, long, value_name = "JSON")]
        view: Option<String>,
        #[arg(help = EDIT, long, value_name = "JSON")]
        tones: Option<String>,
        #[arg(help = EDIT, long, value_name = "JSON")]
        shadows: Option<String>,
        #[arg(help = EDIT, long, value_name = "JSON")]
        path: Option<String>,
        #[arg(help = EDIT, long, value_name = "JSON")]
        new_material: Option<String>,
        #[arg(help = EDIT, long, value_name = "JSON")]
        debug: Option<String>,
    },
    #[command(about = "Show what the models cost at each quality level and on each device tier")]
    Cost3d {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = "Show only this quality level: q0, q1, q2 or q3", long, value_parser = ["q0", "q1", "q2", "q3"])]
        quality: Option<String>,
    },
}
