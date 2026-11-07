use crate::infer::{Device, Opened, open_session, text};
use crate::{LAMA, Places};
use anyhow::{Context, Result, bail};
use image::{GrayImage, RgbImage};
use ort::environment::Environment;
use ort::session::Session;
use ort::value::Tensor;
use std::path::{Path, PathBuf};

const SIDE: usize = crate::windows::CROP as usize;

pub struct Inpainter {
    session: Session,
    device: Device,
    _environment: Environment,
}

impl Inpainter {
    pub fn open(places: &Places, library: Option<&Path>, threads: usize, device: Device) -> Result<Self> {
        let model = places.installed(&LAMA).context("LaMa is not installed; see wprs-ctl models list")?;
        Self::from_file(&model, &places.library(library), threads, device)
    }

    pub fn from_file(model: &Path, libraries: &[PathBuf], threads: usize, device: Device) -> Result<Self> {
        let Opened { session, device, environment } = open_session(model, libraries, threads, device)?;
        Ok(Self { session, device, _environment: environment })
    }

    pub fn device(&self) -> Device {
        self.device
    }

    pub fn run(&mut self, image: &RgbImage, mask: &GrayImage) -> Result<RgbImage> {
        let plane = SIDE * SIDE;
        let mut pixels = vec![0.0f32; 3 * plane];
        for (i, p) in image.pixels().enumerate() {
            (0..3).for_each(|c| pixels[c * plane + i] = f32::from(p[c]) / 255.0);
        }
        let holes: Vec<f32> = mask.pixels().map(|p| f32::from(u8::from(p[0] > 0))).collect();
        let picture = Tensor::from_array(([1usize, 3, SIDE, SIDE], pixels)).map_err(text)?;
        let hole = Tensor::from_array(([1usize, 1, SIDE, SIDE], holes)).map_err(text)?;
        let outputs = self.session.run(ort::inputs!["image" => picture, "mask" => hole]).map_err(text)?;
        let (shape, values) = outputs[0].try_extract_tensor::<f32>().map_err(text)?;
        if shape.len() != 4 || values.len() != 3 * plane {
            bail!("LaMa gave an unexpected shape");
        }
        let byte = |v: f32| v.round().clamp(0.0, 255.0) as u8;
        let interleaved: Vec<u8> =
            (0..plane).flat_map(|i| (0..3).map(move |c| byte(values[c * plane + i]))).collect();
        RgbImage::from_raw(SIDE as u32, SIDE as u32, interleaved).context("LaMa gave a torn picture")
    }
}
