use crate::author::DIR;
use anyhow::Result;
use clap::{ArgAction, Args, Subcommand};
use std::path::PathBuf;
use wprs_editor::actions::{Action, AssetEdit, RgbEdit};
use wprs_editor::{Project, packing};

mod report;

const CUBE: &str = "Make a cube map of this panorama, an equirectangular picture twice as wide as tall";
const FACES: &str = "Make a cube map of six square picture assets of one size, given in this order";
const COVER: &str = "current or previous makes the picture the cover of what the media player plays or played before; none makes it a plain picture again";

#[derive(Args, Debug)]
pub struct AssetArgs {
    #[arg(help = DIR)]
    dir: PathBuf,
    #[arg(help = "Asset id")]
    asset: String,
    #[arg(long, help = "Turn the picture into a KTX2 texture with Basis Universal, as an undoable step")]
    ktx2: bool,
    #[arg(long, requires = "ktx2", conflicts_with = "faces", help = CUBE)]
    cube: bool,
    #[arg(long, requires = "ktx2", value_delimiter = ',', value_name = "+X,-X,+Y,-Y,+Z,-Z", help = FACES)]
    faces: Vec<String>,
    #[arg(long, requires = "ktx2", value_parser = ["uastc", "etc1s"], help = "uastc, sharper (default), or etc1s, smaller")]
    codec: Option<String>,
    #[arg(long, requires = "ktx2", help = "From 1 to 100; 80 when omitted")]
    quality: Option<u32>,
    #[arg(long, requires = "ktx2", help = "Only report the sizes and what each class of graphics card gets")]
    dry_run: bool,
    #[arg(
        long,
        requires = "dry_run",
        value_name = "FOLDER",
        help = "Write a crop of the picture as each class shows it"
    )]
    looks: Option<PathBuf>,
    #[arg(long, conflicts_with = "ktx2", value_parser = wprs_editor::covers::KINDS, help = COVER)]
    cover: Option<String>,
    #[arg(
        long,
        conflicts_with = "ktx2",
        help = "Picture shown while nothing plays: an asset id or a picture of the folder"
    )]
    placeholder: Option<String>,
    #[arg(
        long,
        requires = "cover",
        value_name = "POINTER",
        help = "Show that cover in this field, such as /scene/layers/0/image, in one step: a cover asset of that kind is used again, or a new one is made, and a plain picture never becomes a cover"
    )]
    field: Option<String>,
    #[arg(long, help = crate::cli::JSON)]
    json: bool,
}

#[derive(Subcommand, Debug)]
pub enum Texturing {
    #[command(
        about = "Show a picture asset and what it takes on graphics cards; --ktx2 turns it into a KTX2 texture"
    )]
    Asset(AssetArgs),
    #[command(
        about = "Send what an adjustment layer captures to RGB lighting, or stop, and choose its region"
    )]
    Rgb {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = "Id of an adjustment layer")]
        layer: String,
        #[arg(long, action = ArgAction::Set, help = "true sends and sets permissions.rgb, false stops")]
        on: Option<bool>,
        #[arg(
            long,
            value_name = "WxH",
            conflicts_with = "whole",
            help = "Capture this region around the layer"
        )]
        size: Option<String>,
        #[arg(long, help = "Capture the whole canvas")]
        whole: bool,
    },
}

pub fn run(command: Texturing) -> Result<()> {
    let a = match command {
        Texturing::Asset(args) => args,
        Texturing::Rgb { dir, layer, on, size, whole } => {
            let size = size.map(|s| crate::editing::dimensions(&s)).transpose()?;
            let size = size.map(|(w, h)| [f64::from(w), f64::from(h)]);
            return crate::steps::edit(&dir, Action::Rgb(RgbEdit { layer, on, size, whole }));
        }
    };
    let covered = a.cover.is_some() || a.placeholder.is_some();
    let wanted = AssetEdit {
        asset: a.asset.clone(),
        ktx2: a.ktx2,
        codec: a.codec,
        quality: a.quality,
        cover: a.cover,
        placeholder: a.placeholder,
        field: a.field,
        cube: a.cube,
        faces: a.faces,
    };
    if covered {
        return crate::steps::edit(&a.dir, Action::Asset(wanted));
    }
    let report = match (a.ktx2, a.dry_run) {
        (true, false) => return crate::steps::edit(&a.dir, Action::Asset(wanted)),
        (true, true) => {
            let project = Project::open(&a.dir)?;
            let renderer = a.looks.as_ref().and_then(|_| wprs_render::Renderer::shared().ok());
            let gpu = |bytes: &[u8], target| renderer.and_then(|r| r.look_ktx2(bytes, target));
            let looks = a.looks.as_deref().map(|l| (l, Some(&gpu as packing::Gpu)));
            packing::trial(&project.dir, &project.doc, &wanted, looks)?
        }
        _ => report::shown(&a.dir, &Project::open(&a.dir)?.doc, &a.asset)?,
    };
    println!("{}", if a.json { report.to_string() } else { report::told(&report) });
    Ok(())
}
