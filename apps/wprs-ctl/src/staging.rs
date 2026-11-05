use crate::sowed::pair;
use crate::steps::value;
use clap::Subcommand;
use serde_json::{Map, Value, json};
use wprs_editor::actions::LayerEdit as L;

const XY: &str = "X,Y";

fn mode(text: &str) -> Result<wprs_scene::Blend, String> {
    serde_json::from_value(json!(text)).map_err(|_| format!("{text} is not a blend mode"))
}

#[derive(Subcommand, Debug)]
pub enum Stage {
    #[command(about = "Place, turn, scale, skew or size the layer; what is not given stays")]
    Transform {
        #[arg(long, value_name = XY, value_parser = pair, allow_hyphen_values = true, help = "Pixels of the canvas")]
        position: Option<[f64; 2]>,
        #[arg(long, allow_hyphen_values = true, help = "Degrees from -360 to 360")]
        rotation: Option<f64>,
        #[arg(long, value_name = XY, value_parser = pair, allow_hyphen_values = true, help = "1,1 is the size it has")]
        scale: Option<[f64; 2]>,
        #[arg(long, value_name = XY, value_parser = pair, allow_hyphen_values = true, help = "Degrees")]
        skew: Option<[f64; 2]>,
        #[arg(long, value_name = XY, value_parser = pair, allow_hyphen_values = true, help = "Point it turns and scales around, in fractions of the layer")]
        origin: Option<[f64; 2]>,
        #[arg(allow_hyphen_values = true, long, value_name = "W,H", value_parser = pair, help = "Pixels of an image, video, solid, adjustment or spectrum layer")]
        size: Option<[f64; 2]>,
        #[arg(long, help = crate::sowing::REPLACE)]
        replace: bool,
    },
    #[command(
        about = "Blend the layer over what is below it with a mode such as normal, add, multiply or screen"
    )]
    Blend {
        #[arg(value_parser = mode)]
        blend: wprs_scene::Blend,
    },
    #[command(
        about = "Change one particles layer from the system it draws: alpha, size, count, speed, life, time scale, brightness and color"
    )]
    Instance {
        #[arg(allow_negative_numbers = true, long, help = "From 0 to 1")]
        alpha: Option<f64>,
        #[arg(allow_negative_numbers = true, long, help = "Times the size, from 0 to 100")]
        size: Option<f64>,
        #[arg(allow_negative_numbers = true, long, help = "Share of the particles, from 0 to 1")]
        count: Option<f64>,
        #[arg(allow_negative_numbers = true, long, help = "Times the speed, from 0 to 100")]
        speed: Option<f64>,
        #[arg(allow_negative_numbers = true, long, help = "Times the life, from 0 to 100")]
        life: Option<f64>,
        #[arg(allow_negative_numbers = true, long, help = "Times the pace of time, from 0 to 100")]
        time_scale: Option<f64>,
        #[arg(allow_negative_numbers = true, long, help = "From 0 to 16")]
        brightness: Option<f64>,
        #[arg(long, help = "Color that tints the particles, such as #ffcc88")]
        color: Option<String>,
        #[arg(long, value_name = "VALUE", help = "Draw this value as the system does again")]
        unset: Vec<String>,
        #[arg(long, help = "Draw the layer as the system does in every value")]
        clear: bool,
    },
}

pub(crate) fn edit(layer: String, what: Stage) -> L {
    match what {
        Stage::Transform { position, rotation, scale, skew, origin, size, replace } => {
            L::Transform { layer, position, rotation, scale, skew, origin, size, replace }
        }
        Stage::Blend { blend } => L::Blend { layer, blend },
        Stage::Instance { alpha, size, count, speed, life, time_scale, brightness, color, unset, clear } => {
            let numbers =
                [("alpha", alpha), ("size", size), ("count", count), ("speed", speed), ("life", life)];
            let more = [("time_scale", time_scale), ("brightness", brightness)];
            let mut values: Map<String, Value> =
                numbers.into_iter().chain(more).filter_map(|(k, v)| Some((k.into(), json!(v?)))).collect();
            color.iter().for_each(|c| drop(values.insert("color".into(), value(c))));
            unset.into_iter().for_each(|k| drop(values.insert(k, Value::Null)));
            L::Instance { layer, values, clear }
        }
    }
}
