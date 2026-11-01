use super::{Pixels, SceneAssets};
use crate::{Layer, RenderError, Renderer};
use image::RgbaImage;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use wprs_core::Fit;
use wprs_scene::{Lut, Quality, Runtime, Scene, Values};

const PICTURES: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "bmp"];
const FONTS: [&str; 5] = ["ttf", "otf", "ttc", "otc", "woff2"];
const DECODED: u64 = 512 << 20;

type Failure = Box<dyn std::error::Error + Send + Sync>;

fn still(bytes: Vec<u8>) -> Result<RgbaImage, Failure> {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes));
    Ok(crate::decode_limited(reader, crate::picture_limits())?.into_rgba8())
}

impl SceneAssets {
    pub fn read(
        scene: &Scene,
        mut read: impl FnMut(&str) -> Result<Vec<u8>, String>,
    ) -> Result<SceneAssets, String> {
        let mut assets = SceneAssets::default();
        let mut decoded = crate::Pictures::default();
        for (id, asset) in &scene.assets {
            let ext = Path::new(&asset.path).extension().and_then(|e| e.to_str()).unwrap_or_default();
            let ext = ext.to_ascii_lowercase();
            if PICTURES.contains(&ext.as_str()) {
                let total = decoded
                    .add::<Failure>((&mut assets, id, asset), || Ok(read(&asset.path)?), still)
                    .map_err(|e| e.to_string())?;
                (total <= DECODED)
                    .then_some(())
                    .ok_or("the images of the scene are too large once decoded")?;
            } else if FONTS.contains(&ext.as_str()) {
                assets.fonts.insert(id.clone(), Arc::new(read(&asset.path)?));
            } else if ext == "cube" {
                let lut = Lut::parse(&read(&asset.path)?).map_err(|e| e.to_string())?;
                assets.luts.insert(id.clone(), Arc::new(lut));
            }
        }
        Ok(assets)
    }

    pub fn reloadable<R: FnMut(&str) -> Result<Vec<u8>, String>>(
        &mut self,
        scene: &Scene,
        open: impl Fn() -> Option<R> + Send + Sync + 'static,
    ) {
        let (open, mut made) = (Arc::new(open), HashMap::new());
        for (id, pixels) in &mut self.images {
            let Some(asset) = scene.assets.get(id) else { continue };
            let (path, animated, open) = (asset.path.clone(), asset.animated, open.clone());
            let reload = move || {
                let bytes = open()?(&path).ok()?;
                if animated {
                    crate::animation(&bytes).ok().map(|(atlas, _)| atlas)
                } else {
                    still(bytes).ok()
                }
            };
            let fresh = made.entry(Arc::as_ptr(pixels)).or_insert_with(|| {
                pixels.image().map_or_else(|| pixels.clone(), |image| Pixels::reloadable(image, reload))
            });
            *pixels = fresh.clone();
        }
    }
}

impl Renderer {
    pub fn poster(
        &self,
        scene: &Scene,
        assets: &SceneAssets,
        props: &Values,
        (wall, lang): (f64, &str),
        edge: u32,
    ) -> Result<RgbaImage, RenderError> {
        let [w, h] = scene.canvas.size.map(|v| v.max(1) as f32);
        let scale = (edge as f32 / w.max(h)).min(1.0);
        let size = ((w * scale).round().max(1.0) as u32, (h * scale).round().max(1.0) as u32);
        let mut view = self.scene_view(scene, assets, props)?;
        let mut runtime = Runtime::new(Arc::new(scene.clone()), props.clone());
        assets.sheets.iter().for_each(|(id, d)| runtime.set_sheet(id, d.clone()));
        runtime.set_wall(wall);
        runtime.set_language(lang);
        runtime.seek(f64::from(scene.clock.poster_time));
        let frame = runtime.frame(view.view(size), Quality::Q3);
        let image = self.render_scene(&mut view, &runtime, &frame, size, Quality::Q3, &[])?;
        let (mut target, mut pixels) = (self.create_target(size.0, size.1)?, Vec::new());
        let layer = Layer {
            image: Some(image),
            fit: Fit::Stretch,
            background: [0; 3],
            opacity: 1.0,
            uv: None,
            relief: None,
            flip: [false; 2],
            grade: None,
        };
        self.render_layers(&mut target, &[layer], &mut pixels)?;
        pixels.as_chunks_mut::<4>().0.iter_mut().for_each(|p| p.swap(0, 2));
        RgbaImage::from_raw(size.0, size.1, pixels).ok_or_else(|| RenderError::Readback("short image".into()))
    }
}
