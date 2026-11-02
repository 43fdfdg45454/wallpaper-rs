use crate::author::DIR;
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum Lamping {
    #[command(about = "Add, set, remove, solo or mute a light layer, or plot its falloff")]
    Light {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = "Id of the light layer")]
        layer: String,
        #[command(subcommand)]
        what: Lamp,
    },
    #[command(
        about = "Show where every light is, its handles, the dirty box and the count against each device tier"
    )]
    Lights {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(long, default_value_t = 0.0, help = "Seconds into the scene")]
        time: f64,
        #[arg(long, help = "Show what lighting costs at each quality level instead")]
        cost: bool,
        #[arg(long, value_parser = ["q0", "q1", "q2", "q3"], help = "Quality level the preview uses, with --cost")]
        quality: Option<String>,
    },
    #[command(about = "Set how an image, video or text layer takes the light; an empty id clears a picture")]
    Lighting(Lit),
    #[command(about = "Make a normal map from a picture by its edges and keep it as an asset")]
    Normals {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = "Id of the picture")]
        asset: String,
        #[arg(long, required_unless_present = "preview", help = "Make and keep the normal map")]
        generate: bool,
        #[arg(long, help = "Show the relief of the normal map without keeping it")]
        preview: bool,
        #[arg(long, default_value_t = 2.0, help = "How steep the edges are, 0 to 16")]
        strength: f32,
        #[arg(long, default_value_t = 1.0, help = "Pixels of blur before the edges are found, 0 to 32")]
        blur: f32,
        #[arg(long, help = "An image, video or text layer that takes the normal map")]
        layer: Option<String>,
        #[arg(long, help = "Id of the new asset")]
        id: Option<String>,
    },
    #[command(about = "Paint a roughness or emissive map on a layer in gray, or show it as a heat view")]
    PaintMap {
        #[arg(help = DIR)]
        dir: PathBuf,
        layer: String,
        #[arg(value_parser = ["roughness", "emissive"])]
        map: String,
        #[arg(
            long,
            value_name = "X,Y;X,Y",
            help = "Points from 0 to 1 across the layer",
            required_unless_present = "heat"
        )]
        points: Option<String>,
        #[arg(long, default_value_t = 0.05, help = "Brush radius as a part of the shorter side")]
        radius: f32,
        #[arg(long, default_value_t = 0.5)]
        hardness: f32,
        #[arg(long, default_value_t = 1.0, help = "Gray from 0 to 1")]
        value: f32,
        #[arg(long, help = "Show the map as a heat view instead of painting")]
        heat: bool,
    },
    #[command(about = "Set the light around the scene and from above, or show the hemisphere they make")]
    Ambient {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(long, help = "Color of the light around, such as #202020")]
        ambient: Option<String>,
        #[arg(long, help = "Color of the light from above")]
        sky: Option<String>,
        #[arg(long, help = "Remove both colors")]
        clear: bool,
        #[arg(long, help = "Show the hemisphere instead of changing anything")]
        preview: bool,
    },
}

#[derive(Args, Debug)]
pub struct Lit {
    #[arg(help = DIR)]
    pub dir: PathBuf,
    pub layer: String,
    #[arg(long)]
    pub normal: Option<String>,
    #[arg(long)]
    pub env: Option<String>,
    #[arg(long)]
    pub roughness_map: Option<String>,
    #[arg(long)]
    pub emissive_map: Option<String>,
    #[arg(long, help = "0 to 1")]
    pub roughness: Option<f64>,
    #[arg(long, help = "0 to 1")]
    pub reflection: Option<f64>,
    #[arg(long, help = "0 to 16")]
    pub emissive: Option<f64>,
    #[arg(long, help = "How much the lights count, 0 to 1")]
    pub strength: Option<f64>,
    #[arg(long, help = "Stop lighting the layer")]
    pub off: bool,
}

#[derive(Subcommand, Debug)]
pub enum Lamp {
    #[command(about = "Add a light layer with the defaults of its type")]
    Add {
        #[arg(long = "type", default_value = "point", value_parser = ["point", "spot", "tube", "directional"])]
        light: String,
        #[arg(long, help = "A group to add it to")]
        into: Option<String>,
        #[arg(long, allow_hyphen_values = true, value_name = "X,Y", help = "Position in its group")]
        at: Option<String>,
    },
    #[command(about = "Change a light; --at and --aim are pixels of the canvas")]
    Set(Setting),
    #[command(about = "Remove the light layer")]
    Remove,
    #[command(about = "Light the preview with this light alone, or with all of them again with --off")]
    Solo {
        #[arg(long)]
        off: bool,
    },
    #[command(about = "Leave this light out of the preview, or bring it back with --off")]
    Mute {
        #[arg(long)]
        off: bool,
    },
    #[command(about = "Show the intensity of the light against the distance as points")]
    Falloff {
        #[arg(long, default_value_t = 0.0)]
        time: f64,
    },
}

#[derive(Args, Debug)]
pub struct Setting {
    #[arg(long = "type", value_parser = ["point", "spot", "tube", "directional"])]
    pub light: Option<String>,
    #[arg(long)]
    pub color: Option<String>,
    #[arg(long, help = "0 to 100")]
    pub intensity: Option<f64>,
    #[arg(long, help = "Pixels where the light ends")]
    pub radius: Option<f64>,
    #[arg(long, allow_hyphen_values = true, help = "Height above the canvas in pixels")]
    pub z: Option<f64>,
    #[arg(long, value_name = "INNER,OUTER", help = "Degrees of a spot light")]
    pub cone: Option<String>,
    #[arg(long, help = "Pixels of a tube light")]
    pub length: Option<f64>,
    #[arg(long, allow_hyphen_values = true, value_name = "X,Y")]
    pub at: Option<String>,
    #[arg(long, allow_hyphen_values = true, value_name = "X,Y", help = "A point the light turns to")]
    pub aim: Option<String>,
    #[arg(long, allow_hyphen_values = true, help = "Degrees the beam dips into the canvas")]
    pub tilt: Option<f64>,
}
