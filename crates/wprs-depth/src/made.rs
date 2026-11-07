use crate::infer::{Device, Estimator, Stage, threads};
use crate::{CACHE_BYTES, MODEL, Places, decode, write};
use anyhow::{Context, Result, ensure};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use wprs_core::edit::Depth;

pub struct Bench {
    pub device: Device,
    pub load: Duration,
    pub runs: Vec<Duration>,
}

pub fn bench(places: &Places, image: &Path, depth: &Depth, device: Device, runs: usize) -> Result<Bench> {
    let model =
        places.installed(&MODEL).context("the depth model is not installed; see wprs-ctl models list")?;
    let started = Instant::now();
    let mut estimator = Estimator::open(&model, &places.library(None), threads(), device)?;
    let (pixels, load) = (decode(image)?, started.elapsed());
    let time = |_| {
        let at = Instant::now();
        estimator.estimate(&pixels, depth.input).map(|_| at.elapsed())
    };
    let runs = (0..runs.max(1)).map(time).collect::<Result<Vec<_>>>()?;
    Ok(Bench { device: estimator.device(), load, runs })
}

pub fn generate(
    places: &Places,
    image: (&Path, &str),
    depth: &Depth,
    runtime: (Option<&Path>, Device),
    stage: &mut dyn FnMut(Stage),
) -> Result<PathBuf> {
    generate_until(places, image, depth, runtime, stage, &AtomicBool::new(false))
}

pub fn generate_until(
    places: &Places,
    (image, sha256): (&Path, &str),
    depth: &Depth,
    (library, device): (Option<&Path>, Device),
    stage: &mut dyn FnMut(Stage),
    stop: &AtomicBool,
) -> Result<PathBuf> {
    let model =
        places.installed(&MODEL).context("the depth model is not installed; see wprs-ctl models list")?;
    let fits = (196..=1036).contains(&depth.input) && depth.input.is_multiple_of(14);
    ensure!(fits, "the model input must be a multiple of 14 from 196 to 1036");
    let dest = places.generated(sha256, depth).context("the image has no valid SHA-256")?;
    stage(Stage::Loading);
    let mut estimator = Estimator::open(&model, &places.library(library), threads(), device)?;
    let pixels = decode(image)?;
    stage(Stage::Estimating);
    let map = estimator.estimate_until(&pixels, depth.input, stop)?;
    stage(Stage::Saving);
    write(&map, &dest)?;
    places.trim(&dest, CACHE_BYTES)?;
    Ok(dest)
}
