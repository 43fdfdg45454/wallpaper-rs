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
        #[arg(help = "true to add its motion on top of the layers below", long, action = ArgAction::Set)]
        additive: Option<bool>,
        #[arg(long, help = "Seconds to fade in, 0 to 10")]
        blend_in: Option<f64>,
        #[arg(long, help = "Seconds to fade out, 0 to 10")]
        blend_out: Option<f64>,
        #[arg(help = "false to keep it without playing it", long, action = ArgAction::Set)]
        enabled: Option<bool>,
        #[arg(long, value_delimiter = ',', num_args = 0.., help = "Bones it moves; none for all")]
        bones: Option<Vec<String>>,
        #[arg(long, help = "Move it to this place")]
        to: Option<usize>,
        #[arg(help = "Remove the animation layer", long)]
        remove: bool,
    },
    #[command(about = "Let the root of the first clip move the model layer, or stop it")]
    Root {
        #[arg(help = "true to let the root move the layer", action = ArgAction::Set)]
        on: bool,
    },
    #[command(about = "Swing a bone with a preset: hair, tail, cloth, antenna, chain or jelly")]
    Preset {
        #[arg(help = "The bone to swing")]
        bone: String,
        #[arg(help = "hair, tail, cloth, antenna, chain or jelly")]
        preset: String,
    },
    #[command(about = "Set the weight of a blend shape, -1 to 1, or none to drop it")]
    Morph {
        #[arg(help = "The blend shape")]
        shape: String,
        #[arg(help = "-1 to 1, or none")]
        weight: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum PathCmd {
    #[command(about = "Add a path with two keys where the authoring view looks")]
    Add {
        #[arg(help = "Id of the new camera path")]
        id: String,
    },
    #[command(about = "Set, move or remove a key of a path; without values it takes the authoring view")]
    Key {
        #[arg(help = "The camera path")]
        path: String,
        #[arg(help = "Seconds")]
        time: f64,
        #[arg(help = "Where the camera stands", long, allow_hyphen_values = true, value_name = "X,Y,Z")]
        eye: Option<String>,
        #[arg(
            help = "The point the camera looks at",
            long,
            allow_hyphen_values = true,
            value_name = "X,Y,Z"
        )]
        center: Option<String>,
        #[arg(
            help = "Which way is up for the camera",
            long,
            allow_hyphen_values = true,
            value_name = "X,Y,Z"
        )]
        up: Option<String>,
        #[arg(help = "Field of view in degrees, 1 to 179", long)]
        fov: Option<f64>,
        #[arg(long, value_name = "SECONDS", help = "Move the key at these seconds")]
        from: Option<f64>,
        #[arg(help = "Remove the key at this time", long)]
        remove: bool,
    },
    #[command(about = "Remove a path")]
    Remove {
        #[arg(help = "The camera path")]
        path: String,
    },
    #[command(about = "Loop a path, ease it or rename it")]
    Set {
        #[arg(help = "The camera path")]
        path: String,
        #[arg(help = "true to go back to the first key after the last", id = "loop", long = "loop", action = ArgAction::Set)]
        closed: Option<bool>,
        #[arg(help = "Easing between the keys, such as linear or in_out_sine", long)]
        ease: Option<String>,
        #[arg(help = "A new id for the path", long = "rename")]
        to: Option<String>,
    },
    #[command(about = "Play the paths in order or at random")]
    Order {
        #[arg(help = "Play the paths one after another or at random", value_parser = ["sequential", "random"])]
        order: String,
    },
    #[command(about = "Join the paths through black or by crossing")]
    Fade {
        #[arg(help = "Fade through black or cross from one path to the next", long, value_parser = ["black", "cross"])]
        to: Option<String>,
        #[arg(help = "Seconds of the fade", long)]
        duration: Option<f64>,
        #[arg(help = "Remove the fade", long)]
        clear: bool,
    },
    #[command(about = "Print the paths with their keys and splines in the authoring view")]
    Show {
        #[arg(help = "Seconds into the scene", long, default_value_t = 0.0)]
        time: f64,
    },
}
