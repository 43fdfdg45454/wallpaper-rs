use crate::SceneAssets;
use image::{ImageError, RgbaImage};
use std::collections::HashMap;
use std::sync::Arc;
use wprs_scene::Asset;

pub use wprs_ktx2::moving::MAX_ANIMATION;

pub fn animation(bytes: &[u8]) -> Result<(RgbaImage, Vec<f32>), ImageError> {
    let list = wprs_ktx2::moving::frames(bytes, crate::picture_limits())?;
    let (w, h) = list.first().map_or((1, 1), |(i, _)| i.dimensions());
    let (rows, columns) = crate::scene::layout(list.len());
    let mut atlas = RgbaImage::new(w * columns, h * rows);
    for (i, (image, _)) in list.iter().enumerate() {
        let (x, y) = ((i as u32 % columns) * w, (i as u32 / columns) * h);
        image::imageops::replace(&mut atlas, image, i64::from(x), i64::from(y));
    }
    Ok((atlas, list.into_iter().map(|(_, d)| d).collect()))
}

#[derive(Default)]
pub struct Pictures(HashMap<String, (Arc<crate::Pixels>, Option<Vec<f32>>)>);

impl Pictures {
    pub fn add<E: From<ImageError>>(
        &mut self,
        (assets, id, asset): (&mut SceneAssets, &str, &Asset),
        read: impl FnOnce() -> Result<Vec<u8>, E>,
        still: impl FnOnce(Vec<u8>) -> Result<RgbaImage, E>,
    ) -> Result<u64, E> {
        let key = format!("{}#{}", asset.path, asset.animated);
        let (image, sheet) = match self.0.get(&key) {
            Some(found) => found.clone(),
            None => {
                let bytes = read()?;
                let (image, sheet) = match (asset.animated, wprs_ktx2::is_ktx2(&bytes)) {
                    (true, true) => {
                        let durations = wprs_ktx2::durations(&bytes).map_err(crate::packed::failure)?;
                        (crate::Pixels::ktx2(bytes).map_err(crate::packed::failure)?, Some(durations))
                    }
                    (true, false) => {
                        animation(&bytes).map(|(atlas, d)| (crate::Pixels::new(atlas), Some(d)))?
                    }
                    (false, true) => (crate::Pixels::ktx2(bytes).map_err(crate::packed::failure)?, None),
                    (false, false) => (crate::Pixels::new(still(bytes)?), None),
                };
                self.0.entry(key).or_insert((image, sheet)).clone()
            }
        };
        if let Some(durations) = sheet {
            assets.sheets.insert(id.to_owned(), durations);
        }
        assets.images.insert(id.to_owned(), image);
        Ok(self.0.values().map(|(i, _)| u64::from(i.size().0) * u64::from(i.size().1) * 4).sum())
    }
}
