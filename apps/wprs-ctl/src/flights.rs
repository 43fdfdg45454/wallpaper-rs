use clap::{ArgAction, Subcommand};

#[derive(Subcommand, Debug)]
pub enum AnimCmd {
    #[command(about = "Print the clips of the layer and of its file, its animation layers and root path")]
    Clips,
    #[command(about = "Print the bones and blend shapes the layer can name")]
    Bones,
    #[command(about = "Add, tune, mask, move or remove an animation layer, 0 first")]
    Layer {
        #[arg(long, help = "Add a layer playing this clip")]
        add: Option<String>,
        #[arg(long, help = "The layer changed, 0 first")]
        index: Option<usize>,
        #[arg(long, help = "Play another clip")]
        clip: Option<String>,
        #[arg(long, help = "0 to 1")]
        weight: Option<f64>,
        #[arg(long, help = "Speed, 0 to 16")]
        rate: Option<f64>,
        #[arg(long, action = ArgAction::Set)]
        additive: Option<bool>,
        #[arg(long, help = "Seconds to fade in, 0 to 10")]
        blend_in: Option<f64>,
        #[arg(long, help = "Seconds to fade out, 0 to 10")]
        blend_out: Option<f64>,
        #[arg(long, action = ArgAction::Set)]
        enabled: Option<bool>,
        #[arg(long, value_delimiter = ',', num_args = 0.., help = "Bones it moves; none for all")]
        bones: Option<Vec<String>>,
        #[arg(long, help = "Move it to this place")]
        to: Option<usize>,
        #[arg(long)]
        remove: bool,
    },
    #[command(about = "Let the root of the first clip move the model layer, or stop it")]
    Root {
        #[arg(action = ArgAction::Set)]
        on: bool,
    },
    #[command(about = "Swing a bone with a preset: hair, tail, cloth, antenna, chain or jelly")]
    Preset { bone: String, preset: String },
    #[command(about = "Set the weight of a blend shape, -1 to 1, or none to drop it")]
    Morph { shape: String, weight: String },
}

#[derive(Subcommand, Debug)]
pub enum PathCmd {
    #[command(about = "Add a path with two keys where the authoring view looks")]
    Add { id: String },
    #[command(about = "Set, move or remove a key of a path; without values it takes the authoring view")]
    Key {
        path: String,
        #[arg(help = "Seconds")]
        time: f64,
        #[arg(long, allow_hyphen_values = true, value_name = "X,Y,Z")]
        eye: Option<String>,
        #[arg(long, allow_hyphen_values = true, value_name = "X,Y,Z")]
        center: Option<String>,
        #[arg(long, allow_hyphen_values = true, value_name = "X,Y,Z")]
        up: Option<String>,
        #[arg(long)]
        fov: Option<f64>,
        #[arg(long, value_name = "SECONDS", help = "Move the key at these seconds")]
        from: Option<f64>,
        #[arg(long)]
        remove: bool,
    },
    #[command(about = "Remove a path")]
    Remove { path: String },
    #[command(about = "Loop a path, ease it or rename it")]
    Set {
        path: String,
        #[arg(id = "loop", long = "loop", action = ArgAction::Set)]
        closed: Option<bool>,
        #[arg(long)]
        ease: Option<String>,
        #[arg(long = "rename")]
        to: Option<String>,
    },
    #[command(about = "Play the paths in order or at random")]
    Order {
        #[arg(value_parser = ["sequential", "random"])]
        order: String,
    },
    #[command(about = "Join the paths through black or by crossing")]
    Fade {
        #[arg(long, value_parser = ["black", "cross"])]
        to: Option<String>,
        #[arg(long)]
        duration: Option<f64>,
        #[arg(long)]
        clear: bool,
    },
    #[command(about = "Print the paths with their keys and splines in the authoring view")]
    Show {
        #[arg(long, default_value_t = 0.0)]
        time: f64,
    },
}
