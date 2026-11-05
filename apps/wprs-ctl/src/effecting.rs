use clap::{ArgAction, Subcommand};

const PARAM: &str = "A parameter as KEY=VALUE, the value as JSON or text, such as radius=8 or tint=#ff8800";
const INDEX: &str = "Place of the effect in the stack of the layer, 0 first";

#[derive(Subcommand, Debug)]
pub enum Effect {
    #[command(about = "Add an effect to the stack of the layer, creating the stack when the layer has none")]
    Add {
        #[arg(help = "Id of a built-in effect or of an effect of the package")]
        effect: String,
        #[arg(long, help = "Name of this use, which scripts and bindings call it by")]
        name: Option<String>,
        #[arg(long, help = "Place in the stack; the top when omitted")]
        at: Option<usize>,
        #[arg(long = "param", value_name = "KEY=VALUE", value_parser = crate::effected::keyed, help = PARAM)]
        params: Vec<String>,
        #[arg(long, help = "Leave the effect out instead of the scene where it cannot run")]
        optional: bool,
    },
    #[command(about = "Change the parameters, name or switches of an effect of the layer")]
    Set {
        #[arg(help = INDEX)]
        index: usize,
        #[arg(long = "param", value_name = "KEY=VALUE", value_parser = crate::effected::keyed, help = PARAM)]
        params: Vec<String>,
        #[arg(long, value_name = "KEY", help = "Go back to the default value of a parameter")]
        unset: Vec<String>,
        #[arg(long, action = ArgAction::Set, help = "false turns the effect off without removing it")]
        enabled: Option<bool>,
        #[arg(long, help = "Name of this use; an empty one removes it")]
        name: Option<String>,
        #[arg(long, action = ArgAction::Set, help = "true leaves the effect out where it cannot run")]
        optional: Option<bool>,
    },
    #[command(about = "Remove an effect from the stack of the layer")]
    Remove {
        #[arg(help = INDEX)]
        index: usize,
    },
    #[command(about = "Move an effect to another place in the stack of the layer")]
    Move {
        #[arg(help = INDEX)]
        index: usize,
        #[arg(help = "New place, 0 first")]
        to: usize,
    },
    #[command(
        about = "Limit an effect to part of the layer with a picture, another layer or a shape, or remove the mask with --off"
    )]
    Mask {
        #[arg(help = INDEX)]
        index: usize,
        #[arg(long, conflicts_with_all = ["from", "shape"], help = "Picture asset whose channel masks the effect")]
        texture: Option<String>,
        #[arg(
            long,
            value_name = "LAYER",
            conflicts_with = "shape",
            help = "Layer whose picture masks the effect"
        )]
        from: Option<String>,
        #[arg(long, help = "Shape as JSON, such as '{\"ellipse\": {\"size\": [400, 300]}}'")]
        shape: Option<String>,
        #[arg(long, value_parser = ["r", "g", "b", "a", "luminance"], help = "Channel that masks; alpha when omitted")]
        channel: Option<String>,
        #[arg(long, action = ArgAction::Set, help = "true masks the outside instead")]
        invert: Option<bool>,
        #[arg(allow_negative_numbers = true, long, help = "Pixels the edge of a shape fades over")]
        feather: Option<f64>,
        #[arg(allow_negative_numbers = true, long, help = "How much the mask holds back, from 0 to 1")]
        strength: Option<f64>,
        #[arg(long, conflicts_with_all = ["texture", "from", "shape"], help = "Remove the mask")]
        off: bool,
    },
}
