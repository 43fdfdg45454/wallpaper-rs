use crate::{Device, Inpaint, Inpainter, Places, Settings, background, load, threads};
use anyhow::{Context, Result};
use image::{GrayImage, RgbImage};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use wprs_core::edit::{Depth, Edit, Parallax};
use wprs_core::{Config, Fill};

pub fn settings(places: &Places, edit: &Edit, fill: Option<Fill>) -> Settings {
    let config = Config::default_path().and_then(|p| Config::load(&p)).unwrap_or_default().parallax;
    let shift = edit.parallax.as_ref().map_or(Parallax::default().strength, |p| p.strength);
    Settings { fill: places.fill(fill.unwrap_or(config.fill)), count: usize::from(config.layers), shift }
}

impl Places {
    pub fn ensure_layers(
        &self,
        (image, sha): (&Path, &str),
        (depth, set): (&Depth, &Settings),
        (device, force): (Device, bool),
        progress: &mut dyn FnMut(u64),
        stop: &AtomicBool,
    ) -> Result<PathBuf> {
        let dest = self.layer_set(image, sha, depth, set);
        let dest = dest.context("the image has no depth map; generate or import one first")?;
        if dest.is_file() && !force && load(&dest).is_ok_and(|l| l.is_some()) {
            return Ok(dest);
        }
        background();
        let mut lama =
            (set.fill == Fill::Lama).then(|| Inpainter::open(self, None, threads(), device)).transpose()?;
        let mut paint = |image: &RgbImage, mask: &GrayImage| {
            lama.as_mut().context("LaMa is not loaded")?.run(image, mask)
        };
        let inpaint: Option<&mut Inpaint<'_>> = (set.fill == Fill::Lama).then_some(&mut paint);
        self.build_layers((image, sha), (depth, set), inpaint, progress, stop)
    }
}
