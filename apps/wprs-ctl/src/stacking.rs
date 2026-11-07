use crate::depth::key;
use crate::editing::open;
use crate::usage;
use anyhow::{Context, Result, ensure};
use image::{GrayImage, RgbImage};
use std::path::PathBuf;
use wprs_core::edit::Parallax;
use wprs_core::{Config, Fill};
use wprs_depth::{Device, Inpainter, Places, Settings, background, load, threads};

#[derive(clap::Args, Debug)]
pub struct Make {
    #[arg(help = "Image file")]
    path: PathBuf,
    #[arg(long, value_parser = ["lama", "simple", "off"], help = "How the background behind near objects is filled; the setting parallax.fill by default")]
    fill: Option<String>,
    #[arg(long, value_parser = clap::value_parser!(u8).range(4..=6), help = "Number of depth layers, 4 to 6; the setting parallax.layers by default")]
    layers: Option<u8>,
    #[arg(long, help = "Build them again even when the set exists")]
    force: bool,
    #[arg(long, conflicts_with_all = ["clear", "force"], help = "List the layer sets of the image instead of building one")]
    list: bool,
    #[arg(long, conflicts_with = "force", help = "Delete the layer sets of the image")]
    clear: bool,
    #[arg(long, default_value = "auto", value_parser = ["auto", "cpu", "cuda"], help = "Where LaMa runs: cuda needs an ONNX Runtime built with CUDA")]
    device: String,
}

#[derive(clap::Args, Debug)]
pub struct Peek {
    #[arg(help = "Image file")]
    path: PathBuf,
    #[arg(long, help = "Layer to save, 0 for the farthest")]
    layer: usize,
    #[arg(long, value_name = "FILE", help = "PNG file to write, with the transparency of the layer")]
    out: PathBuf,
}

pub fn make(args: Make) -> Result<()> {
    let places = Places::default();
    let edit = open(&args.path)?;
    let (sha, depth) = (key(&args.path, &edit)?, edit.depth.clone().unwrap_or_default());
    if args.clear {
        let removed = places.unlayer(&sha);
        ensure!(removed > 0, "the image has no depth layers");
        println!("{removed} layer sets removed");
        return Ok(());
    }
    if args.list {
        for path in places.layers_of(&sha) {
            let info = load(&path)?.map(|l| l.info);
            let (fill, count) = info.map_or((String::new(), 0), |i| (i.fill, i.depths.len()));
            println!("{}  {count} layers, fill {fill}", path.display());
        }
        return Ok(());
    }
    let config = Config::default_path().and_then(|p| Config::load(&p)).unwrap_or_default().parallax;
    let wanted = args.fill.as_deref().map_or(Ok(config.fill), Fill::parse).map_err(usage)?;
    ensure!(wanted != Fill::Off, "the fill off builds no layers: pictures keep the single image warp");
    let fill = places.fill(wanted);
    if fill != wanted {
        eprintln!("LaMa or ONNX Runtime is not installed: using the simple fill (see wprs-ctl models list)");
    }
    let shift = edit.parallax.as_ref().map_or(Parallax::default().strength, |p| p.strength);
    let set = Settings { fill, count: usize::from(args.layers.unwrap_or(config.layers)), shift };
    let dest = places.layer_set(&args.path, &sha, &depth, &set);
    let dest = dest.context("the image has no depth map; generate or import one first")?;
    if dest.is_file() && !args.force && load(&dest).is_ok_and(|l| l.is_some()) {
        println!("{}", dest.display());
        return Ok(());
    }
    background();
    let mut lama = (fill == Fill::Lama)
        .then(|| Inpainter::open(&places, None, threads(), args.device.parse().unwrap_or(Device::Auto)))
        .transpose()?;
    let mut paint =
        |image: &RgbImage, mask: &GrayImage| lama.as_mut().context("LaMa is not loaded")?.run(image, mask);
    let inpaint: Option<&mut wprs_depth::Inpaint<'_>> = (fill == Fill::Lama).then_some(&mut paint);
    let mut shown = u64::MAX;
    let mut progress = |done: u64| {
        if done / 10 != shown / 10 {
            eprintln!("building the layers: {done} %");
        }
        shown = done;
    };
    let stop = std::sync::atomic::AtomicBool::new(false);
    let made = places.build_layers((&args.path, &sha), (&depth, &set), inpaint, &mut progress, &stop)?;
    println!("{}", made.display());
    Ok(())
}

pub fn peek(args: Peek) -> Result<()> {
    let places = Places::default();
    let edit = open(&args.path)?;
    let sets = places.layers_of(&key(&args.path, &edit)?);
    let newest = sets.into_iter().max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    let path = newest.context("the image has no depth layers; build them with edit depth layers")?;
    let set = load(&path)?.context("the layer set is damaged; build it again with --force")?;
    let count = set.images.len();
    let layer = set
        .images
        .get(args.layer)
        .with_context(|| format!("the image has {count} layers, 0 to {}", count - 1))?;
    layer.save_with_format(&args.out, image::ImageFormat::Png)?;
    println!("{}", args.out.display());
    Ok(())
}
