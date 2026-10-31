use crate::editing::open;
use anyhow::{Context, Result};
use clap::Subcommand;
use std::path::{Path, PathBuf};
use wprs_core::edit::{Action, Depth, Edit, Source};
use wprs_depth::{Device, Places, Stage, adjust, beside, coloured, read, write};

const MAX_EDGE: u32 = 4096;

#[derive(Subcommand, Debug)]
pub enum Maps {
    #[command(about = "Estimate the depth map with the downloaded model, showing progress; Ctrl-C cancels")]
    Generate {
        path: PathBuf,
        #[arg(long, help = "Estimate it again even when a map exists")]
        force: bool,
        #[arg(long, default_value = "auto", value_parser = ["auto", "cpu", "cuda"], help = "Where the model runs: cuda needs an ONNX Runtime built with CUDA, auto uses it when it can")]
        device: String,
        #[arg(long, value_name = "RUNS", num_args = 0..=1, default_missing_value = "3", help = "Time the model instead of saving a map")]
        bench: Option<usize>,
    },
    #[command(about = "Use a picture as the depth map, saved next to the image; white is near")]
    Import { path: PathBuf, file: PathBuf },
    #[command(
        about = "Save the depth map as a 16-bit grey PNG; next to the image without a file, except with --adjusted or --colour, which need one"
    )]
    Export {
        path: PathBuf,
        file: Option<PathBuf>,
        #[arg(long, help = "With the near, far, curve, smoothing and growth of the edit")]
        adjusted: bool,
        #[arg(long, help = "In false colour, red near and blue far, as an 8-bit PNG")]
        colour: bool,
    },
}

#[derive(clap::Args, Debug)]
pub struct Levels {
    #[arg(long, help = "Level of the nearest point, 0 to 1, 1 by default")]
    near: Option<f32>,
    #[arg(long, help = "Level of the farthest point, 0 to 1, 0 by default")]
    far: Option<f32>,
    #[arg(long, help = "Curve between them, 0.2 to 5, 1 by default")]
    gamma: Option<f32>,
    #[arg(long, help = "Blur radius in pixels of the map, 0 to 16, 2 by default")]
    smooth: Option<u32>,
    #[arg(long, help = "Pixels by which near objects grow over their edges, 0 to 16, 3 by default")]
    dilate: Option<u32>,
    #[arg(long, help = "Short side given to the model: a multiple of 14 from 196 to 1036, 518 by default")]
    input: Option<u32>,
}

#[derive(clap::Args, Debug)]
pub struct Look {
    #[arg(long, value_parser = ["auto", "pointer", "accelerometer"], help = "What moves it; auto when new")]
    source: Option<String>,
    #[arg(long, help = "Largest shift as a fraction of the width, up to 0.03")]
    strength: Option<f32>,
    #[arg(long, help = "Depth that stays still, 0 far to 1 near")]
    focus: Option<f32>,
    #[arg(long, value_parser = ["1", "8", "16", "32"], help = "Quality: 1 shifts, more steps follow edges")]
    steps: Option<String>,
    #[arg(long, value_name = "MS", help = "Time the movement takes to settle, 0 to 2000 ms")]
    smoothing: Option<u32>,
    #[arg(long, help = "Zoom that hides the edges, 1 to 1.1")]
    overscan: Option<f32>,
    #[arg(long, help = "Turn the parallax off and keep its settings")]
    off: bool,
}

impl Levels {
    pub fn action(self) -> Action {
        let Levels { near, far, gamma, smooth, dilate, input } = self;
        Action::Depth { near, far, gamma, smooth, dilate, input }
    }
}

impl Look {
    pub fn action(self) -> Action {
        let Look { source, strength, focus, steps, smoothing, overscan, off } = self;
        let steps = steps.and_then(|s| s.parse().ok());
        Action::Parallax { source, strength, focus, steps, smoothing, overscan, off }
    }
}

pub fn key(path: &Path, edit: &Edit) -> Result<String> {
    match edit.source.sha256.as_str() {
        "" => Ok(Source::read(path, [0; 2])?.sha256),
        sha => Ok(sha.to_owned()),
    }
}

pub fn map(path: &Path, edit: &Edit) -> Result<Option<wprs_depth::Map>> {
    let depth = edit.depth.clone().unwrap_or_default();
    Places::default().map(path, &key(path, edit)?, &depth, MAX_EDGE)
}

pub fn run(command: Maps) -> Result<()> {
    let places = Places::default();
    let (path, edit) = match &command {
        Maps::Generate { path, .. } | Maps::Import { path, .. } | Maps::Export { path, .. } => {
            (path, open(path)?)
        }
    };
    let (sha, depth) = (key(path, &edit)?, edit.depth.clone().unwrap_or_default());
    let found = places.find(path, &sha, &depth);
    let shown = match command {
        Maps::Generate { path, device, bench: Some(runs), .. } => {
            return bench(&places, &path, &depth, &device, runs);
        }
        Maps::Generate { force: false, .. } if found.is_some() => found,
        Maps::Generate { path, device, .. } => Some(generate(&places, &path, &sha, &depth, device.parse()?)?),
        Maps::Import { path, file } => Some(places.import(&file, &path, &sha)?),
        Maps::Export { path, file, adjusted, colour } => {
            let from = found.context("the image has no depth map; generate or import one first")?;
            let to = match file {
                Some(file) => file,
                None if adjusted || colour => {
                    return Err(crate::usage("name the file: --adjusted and --colour never replace the map"));
                }
                None => beside(&path),
            };
            let map = if adjusted { adjust(&read(&from)?, &depth) } else { read(&from)? };
            match colour {
                true => coloured(&map, true).save_with_format(&to, image::ImageFormat::Png)?,
                false => write(&map, &to)?,
            }
            Some(to)
        }
    };
    shown.iter().for_each(|p| println!("{}", p.display()));
    Ok(())
}

fn bench(places: &Places, path: &Path, depth: &Depth, device: &str, runs: usize) -> Result<()> {
    let found = wprs_depth::bench(places, path, depth, device.parse()?, runs)?;
    let mut times: Vec<f64> = found.runs.iter().map(|t| t.as_secs_f64()).collect();
    times.sort_by(f64::total_cmp);
    let (median, fastest) = (times[times.len() / 2], times[0]);
    println!(
        "{}: loaded in {:.2} s, estimated at {} in {median:.3} s (median of {}, fastest {fastest:.3} s)",
        found.device.name(),
        found.load.as_secs_f64(),
        depth.input,
        times.len()
    );
    Ok(())
}

fn generate(places: &Places, path: &Path, sha: &str, depth: &Depth, device: Device) -> Result<PathBuf> {
    wprs_depth::background();
    let started = std::time::Instant::now();
    let made = wprs_depth::generate(places, (path, sha), depth, (None, device), &mut |stage| match stage {
        Stage::Loading => eprintln!("loading the depth model"),
        Stage::Estimating => eprintln!("estimating depth"),
        Stage::Saving => eprintln!("saving the depth map"),
    })?;
    eprintln!("done in {:.1} s", started.elapsed().as_secs_f32());
    Ok(made)
}
