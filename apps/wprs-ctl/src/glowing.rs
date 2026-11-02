use clap::Args;
use std::path::Path;
use wprs_editor::Project;
use wprs_editor::actions::{LightEdit, SwarmLight};

#[derive(Args, Debug)]
pub struct Air {
    #[arg(long, help = "How far the light reaches into the air, 0 to 1")]
    pub density: Option<f64>,
    #[arg(long, help = "Steps along each ray, 4 to 64")]
    pub samples: Option<u32>,
    #[arg(long, help = "Stop lighting the air")]
    pub off: bool,
    #[arg(long, help = "Show the glow against the distance and what each quality level keeps instead")]
    pub show: bool,
    #[arg(long, default_value_t = 0.0)]
    pub time: f64,
}

#[derive(Args, Debug)]
pub struct Cast {
    #[arg(long, conflicts_with = "from", help = "A picture or video of the scene")]
    pub asset: Option<String>,
    #[arg(long = "layer", help = "A layer, drawn with its effects; it must not take light itself")]
    pub from: Option<String>,
    #[arg(long, help = "Stop projecting")]
    pub off: bool,
    #[arg(long, help = "Show what can be projected and where the picture lands instead")]
    pub show: bool,
    #[arg(long, default_value_t = 0.0)]
    pub time: f64,
}

#[derive(Args, Debug)]
pub struct Lit {
    #[arg(long, help = "A normal map that turns each sprite; an empty id clears it")]
    pub normal: Option<String>,
    #[arg(long, help = "Stop lighting the particles")]
    pub off: bool,
    #[arg(long, help = "A particles layer whose own render changes")]
    pub layer: Option<String>,
    #[arg(long, help = "Show the two-sided preview and what each quality level keeps instead")]
    pub show: bool,
}

#[derive(Args, Debug)]
pub struct Bend {
    #[arg(long, help = "A normal map that bends what is behind each sprite")]
    pub normal: Option<String>,
    #[arg(long, help = "How far it bends, 0 to 1 of half the sprite")]
    pub amount: Option<f64>,
    #[arg(long, help = "Stop refracting")]
    pub off: bool,
    #[arg(long, help = "A particles layer whose own render changes")]
    pub layer: Option<String>,
    #[arg(long, help = "Show the displacement and what each quality level keeps instead")]
    pub show: bool,
}

pub(crate) fn air(layer: String, a: &Air) -> LightEdit {
    LightEdit::Volumetric { layer, density: a.density, samples: a.samples, off: a.off }
}

pub(crate) fn cast(layer: String, c: &Cast) -> LightEdit {
    LightEdit::Projection { layer, asset: c.asset.clone(), from: c.from.clone(), off: c.off }
}

pub(crate) fn lit(system: String, l: &Lit) -> SwarmLight {
    let layer = l.layer.clone().filter(|l| !l.is_empty());
    SwarmLight::Lit { system, layer, lit: Some(!l.off), normal: l.normal.clone() }
}

pub(crate) fn bend(system: String, b: &Bend) -> SwarmLight {
    let layer = b.layer.clone().filter(|l| !l.is_empty());
    SwarmLight::Refract { system, layer, normal: b.normal.clone(), amount: b.amount, off: b.off }
}

pub(crate) fn print(dir: &Path, show: impl Fn(&Project) -> serde_json::Value) -> anyhow::Result<()> {
    let found = show(&Project::open(dir)?);
    anyhow::ensure!(!found.is_null(), "there is nothing to show");
    println!("{}", serde_json::to_string_pretty(&found)?);
    Ok(())
}
