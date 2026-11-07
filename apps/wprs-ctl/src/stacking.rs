use crate::depth::key;
use crate::editing::open;
use crate::usage;
use anyhow::{Context, Result, ensure};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use wprs_core::edit::{Depth, Edit};
use wprs_core::{Config, Fill};
use wprs_depth::{Device, Layers, Places, Settings, band_mask, halves, load, settings};

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
    #[arg(
        long,
        value_name = "FILE",
        help = "Also write the band of the layer that moving can uncover, white on black, as a PNG"
    )]
    band: Option<PathBuf>,
    #[arg(long, value_parser = ["lama", "simple"], help = "Show the set made with this fill instead of the newest one")]
    fill: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct Compare {
    #[arg(help = "Image file")]
    path: PathBuf,
    #[arg(
        long,
        value_name = "FILE",
        help = "PNG file to write: the layer filled by LaMa on the left, by the simple fill on the right"
    )]
    out: PathBuf,
    #[arg(long, help = "Layer to compare, 0 for the farthest; the one with the largest band by default")]
    layer: Option<usize>,
    #[arg(
        long,
        default_value_t = 0.5,
        help = "Where the picture changes from LaMa to the simple fill, 0 to 1"
    )]
    at: f32,
    #[arg(long, default_value = "auto", value_parser = ["auto", "cpu", "cuda"], help = "Where LaMa runs")]
    device: String,
}

fn ensure_set(
    places: &Places,
    edit: &Edit,
    wanted: Option<Fill>,
    (path, sha): (&Path, &str),
    device: Device,
) -> Result<(PathBuf, Settings)> {
    let set = settings(places, edit, wanted);
    let depth = edit.depth.clone().unwrap_or_default();
    let mut shown = u64::MAX;
    let mut progress = |done: u64| {
        if done / 10 != shown / 10 {
            eprintln!("building the {} layers: {done} %", set.fill.as_str());
        }
        shown = done;
    };
    let stop = AtomicBool::new(false);
    let made = places.ensure_layers((path, sha), (&depth, &set), (device, false), &mut progress, &stop)?;
    Ok((made, set))
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
    let mut set = settings(&places, &edit, Some(wanted));
    if set.fill != wanted {
        eprintln!("LaMa or ONNX Runtime is not installed: using the simple fill (see wprs-ctl models list)");
    }
    set.count = usize::from(args.layers.unwrap_or(config.layers));
    let device = args.device.parse().unwrap_or(Device::Auto);
    let mut shown = u64::MAX;
    let mut progress = |done: u64| {
        if done / 10 != shown / 10 {
            eprintln!("building the layers: {done} %");
        }
        shown = done;
    };
    let stop = AtomicBool::new(false);
    let sets = (&depth, &set);
    let made = places.ensure_layers((&args.path, &sha), sets, (device, args.force), &mut progress, &stop)?;
    println!("{}", made.display());
    Ok(())
}

fn pick(places: &Places, sha: &str, fill: Option<&str>) -> Result<Layers> {
    let dated = |p: PathBuf| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p));
    let mut sets: Vec<_> = places.layers_of(sha).into_iter().filter_map(dated).collect();
    sets.sort();
    let found = sets.into_iter().rev().filter_map(|(_, path)| load(&path).ok().flatten());
    let mut found = found.filter(|set| fill.is_none_or(|word| set.info.fill == word));
    found.next().with_context(|| match fill {
        Some(word) => {
            format!("the image has no layers filled with {word}; build them with edit depth layers")
        }
        None => "the image has no depth layers; build them with edit depth layers".into(),
    })
}

pub fn peek(args: Peek) -> Result<()> {
    let places = Places::default();
    let edit = open(&args.path)?;
    let sha = key(&args.path, &edit)?;
    let set = pick(&places, &sha, args.fill.as_deref())?;
    let count = set.images.len();
    let layer = set.images.get(args.layer);
    let layer = layer.with_context(|| format!("the image has {count} layers, 0 to {}", count - 1))?;
    layer.save_with_format(&args.out, image::ImageFormat::Png)?;
    println!("{}", args.out.display());
    if let Some(file) = &args.band {
        let depth: Depth = edit.depth.clone().unwrap_or_default();
        let (found, size) = places.layer_band((&args.path, &sha), &depth, (&set.info, args.layer))?;
        band_mask(&found, size).save_with_format(file, image::ImageFormat::Png)?;
        println!("{}", file.display());
    }
    Ok(())
}

pub fn compare(args: Compare) -> Result<()> {
    ensure!((0.0..=1.0).contains(&args.at), "--at is a fraction from 0 to 1");
    let places = Places::default();
    let edit = open(&args.path)?;
    let (sha, device) = (key(&args.path, &edit)?, args.device.parse().unwrap_or(Device::Auto));
    let mut sets = Vec::new();
    for fill in [Fill::Lama, Fill::Simple] {
        let (path, set) = ensure_set(&places, &edit, Some(fill), (&args.path, &sha), device)?;
        ensure!(set.fill == fill, "LaMa or ONNX Runtime is not installed: see wprs-ctl models list");
        sets.push(load(&path)?.context("the layer set is damaged")?);
    }
    let count = sets[0].images.len().min(sets[1].images.len());
    let widest = (0..count).max_by_key(|k| sets[0].info.bands[*k].pixels).unwrap_or(0);
    let layer = args.layer.unwrap_or(widest);
    ensure!(layer < count, "the sets have {count} layers, 0 to {}", count - 1);
    halves(&sets[0].images[layer], &sets[1].images[layer], args.at)
        .save_with_format(&args.out, image::ImageFormat::Png)?;
    println!("{}", args.out.display());
    Ok(())
}
