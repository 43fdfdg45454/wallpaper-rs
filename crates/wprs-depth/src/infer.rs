use crate::{CACHE_BYTES, MODEL, Map, Places, normalize, tensor, write};
use anyhow::{Context, Result, anyhow, bail, ensure};
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader, RgbImage};
use ort::environment::Environment;
use ort::session::Session;
use ort::value::Tensor;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use wprs_core::edit::Depth;

const MAX_EDGE: u32 = 2048;
pub const TELEMETRY: &str = "ORT_DISABLE_TELEMETRY";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Loading,
    Estimating,
    Saving,
}

pub struct Estimator {
    session: Session,
    _environment: Environment,
}

fn text(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow!("{e}")
}

pub fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().saturating_sub(1).max(1))
}

pub fn background() {
    unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, 19) };
}

fn environment(libraries: &[PathBuf]) -> Result<Environment> {
    static SHARED: OnceLock<Environment> = OnceLock::new();
    if let Some(environment) = SHARED.get() {
        return Ok(environment.clone());
    }
    let mut failure = anyhow!("ONNX Runtime is not installed; see wprs-ctl models list");
    unsafe { std::env::set_var(TELEMETRY, "1") };
    let loaded = libraries.iter().find_map(|lib| {
        ort::init_from(lib).inspect_err(|e| failure = anyhow!("cannot load ONNX Runtime: {e}")).ok()
    });
    let built =
        loaded.ok_or(failure)?.with_name("wallpaper-rs").with_telemetry(false).build().map_err(text)?;
    Ok(SHARED.get_or_init(|| built).clone())
}

impl Estimator {
    pub fn open(model: &Path, libraries: &[PathBuf], threads: usize) -> Result<Self> {
        let environment = environment(libraries)?;
        let cpu = ort::ep::CPU::default().with_arena_allocator(false).build();
        let session = Session::builder(&environment)
            .map_err(text)?
            .with_execution_providers([cpu])
            .map_err(text)?
            .with_intra_threads(threads)
            .map_err(text)?
            .with_memory_pattern(false)
            .map_err(text)?
            .commit_from_file(model)
            .map_err(text)?;
        Ok(Self { session, _environment: environment })
    }

    pub fn estimate(&mut self, image: &RgbImage, input: u32) -> Result<Map> {
        let (data, (w, h)) = tensor(image, input);
        let value = Tensor::from_array(([1usize, 3, h as usize, w as usize], data)).map_err(text)?;
        let outputs = self.session.run(ort::inputs![value]).map_err(text)?;
        let (shape, values) = outputs[0].try_extract_tensor::<f32>().map_err(text)?;
        let &[.., oh, ow] = &shape[..] else { bail!("the model gave no depth") };
        Ok(normalize(values, (ow as u32, oh as u32), image.dimensions(), input))
    }
}

pub fn decode(path: &Path) -> Result<RgbImage> {
    let reader = ImageReader::open(path)?.with_guessed_format()?;
    let mut decoder =
        reader.into_decoder().with_context(|| format!("{} is not a picture", path.display()))?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder)?;
    if image.width().max(image.height()) > MAX_EDGE {
        image = image.thumbnail(MAX_EDGE, MAX_EDGE);
    }
    image.apply_orientation(orientation);
    Ok(image.into_rgb8())
}

pub fn generate(
    places: &Places,
    (image, sha256): (&Path, &str),
    depth: &Depth,
    library: Option<&Path>,
    stage: &mut dyn FnMut(Stage),
) -> Result<PathBuf> {
    let model =
        places.installed(&MODEL).context("the depth model is not installed; see wprs-ctl models list")?;
    let fits = (196..=1036).contains(&depth.input) && depth.input.is_multiple_of(14);
    ensure!(fits, "the model input must be a multiple of 14 from 196 to 1036");
    let dest = places.generated(sha256, depth).context("the image has no valid SHA-256")?;
    stage(Stage::Loading);
    let mut estimator = Estimator::open(&model, &places.library(library), threads())?;
    let pixels = decode(image)?;
    stage(Stage::Estimating);
    let map = estimator.estimate(&pixels, depth.input)?;
    stage(Stage::Saving);
    write(&map, &dest)?;
    places.trim(&dest, CACHE_BYTES)?;
    Ok(dest)
}
