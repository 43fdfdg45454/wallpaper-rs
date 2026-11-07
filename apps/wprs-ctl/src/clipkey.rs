use crate::author::{DIR, ID};
use crate::rigging::pair;
use crate::steps::value;
use clap::Args;
use std::path::PathBuf;
use wprs_editor::actions::{Action, ClipKey};

#[derive(Args, Debug)]
pub struct KeyArgs {
    #[arg(help = DIR)]
    dir: PathBuf,
    #[arg(help = ID)]
    layer: String,
    #[arg(help = "Id of the clip")]
    clip: String,
    #[arg(allow_negative_numbers = true, help = "Seconds from the start of the clip")]
    time: f64,
    #[arg(long, conflicts_with_all = ["shape", "order"], help = "The bone keyed")]
    bone: Option<String>,
    #[arg(long, allow_hyphen_values = true, help = "Degrees clockwise added to its rest pose")]
    rotation: Option<f32>,
    #[arg(
        long,
        allow_hyphen_values = true,
        value_name = "X,Y",
        help = "Pixels, y down, in the space of its parent"
    )]
    position: Option<String>,
    #[arg(long, allow_hyphen_values = true, value_name = "S or X,Y", help = "Scale of its rest pose")]
    scale: Option<String>,
    #[arg(long, allow_hyphen_values = true, value_name = "X,Y,Z", help = "Degrees around x, y and z")]
    angles: Option<String>,
    #[arg(long, allow_hyphen_values = true, value_name = "X,Y,Z", help = "Model units moved")]
    shift: Option<String>,
    #[arg(long, conflicts_with = "order", help = "The blend shape keyed")]
    shape: Option<String>,
    #[arg(long, allow_hyphen_values = true, help = "Weight of the blend shape, from -1 to 1")]
    weight: Option<f32>,
    #[arg(long, help = "The part whose order is keyed")]
    order: Option<String>,
    #[arg(long, allow_hyphen_values = true, help = "Order of the part, from -1000 to 1000")]
    value: Option<f32>,
    #[arg(
        long,
        help = "How the key moves on to the next one: linear, step, hold, in_out_sine and the like, or [x1,y1,x2,y2]"
    )]
    ease: Option<String>,
    #[arg(long, allow_hyphen_values = true, value_name = "DT,DV", value_parser = crate::keying::pair, help = crate::keying::HANDLE_IN)]
    in_handle: Option<[f64; 2]>,
    #[arg(long, allow_hyphen_values = true, value_name = "DT,DV", value_parser = crate::keying::pair, help = crate::keying::HANDLE_OUT)]
    out_handle: Option<[f64; 2]>,
    #[arg(long, help = "Remove the key")]
    remove: bool,
    #[arg(
        allow_negative_numbers = true,
        long,
        value_name = "SECONDS",
        help = "Move the key that is at these seconds"
    )]
    from: Option<f64>,
    #[arg(long, requires = "from", help = "Copy the key instead of moving it")]
    copy: bool,
}

impl KeyArgs {
    pub(crate) fn action(self) -> (PathBuf, Action) {
        let angles = self.angles.as_deref().and_then(crate::animated::three);
        let shift = self.shift.as_deref().and_then(crate::animated::three);
        let position = self.position.as_deref().and_then(pair);
        let scale =
            self.scale.as_deref().and_then(|s| pair(s).or_else(|| s.trim().parse().ok().map(|v| [v, v])));
        let key = ClipKey {
            layer: self.layer,
            clip: self.clip,
            time: self.time,
            bone: self.bone,
            rotation: self.rotation,
            position,
            scale,
            angles,
            shift,
            shape: self.shape,
            weight: self.weight,
            order: self.order,
            value: self.value,
            ease: self.ease.as_deref().map(value),
            in_handle: self.in_handle,
            out_handle: self.out_handle,
            remove: self.remove,
            from: self.from,
            copy: self.copy,
        };
        (self.dir, Action::Key(key))
    }
}
