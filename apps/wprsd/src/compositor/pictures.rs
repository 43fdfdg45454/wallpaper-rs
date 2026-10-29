use super::output::{Output, Raised};
use super::{Content, Picture};
use image::RgbaImage;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use wprs_core::edit::Edit;
use wprs_core::{Fit, UvRect};
use wprs_render::{GpuImage, Renderer, decode_image, decoded_size, needed_edge};

pub(super) type Look =
    (PathBuf, Option<SystemTime>, Option<Arc<Edit>>, Option<(PathBuf, Option<SystemTime>)>);
pub(super) type Made = (GpuImage, UvRect, Option<Raised>);
type Target = (Fit, (u32, u32));

#[derive(Clone)]
pub(super) struct Developed {
    pub(super) look: Look,
    pub(super) image: GpuImage,
    pub(super) relief: Option<Raised>,
}

#[derive(Default)]
pub(super) struct Pictures(Vec<Developed>);

pub(super) fn look(picture: &Picture) -> Look {
    let depth = picture.depth.as_ref().map(|d| (d.file.clone(), d.modified));
    (picture.file.clone(), picture.modified, picture.edit.clone(), depth)
}

fn serves(have: (u32, u32), need: (u32, u32)) -> bool {
    let fine = |h: u32, n: u32| u64::from(h) * 100 >= u64::from(n) * 99 && h <= n.saturating_mul(2);
    fine(have.0, need.0) && fine(have.1, need.1)
}

fn showing(output: &Output, look: &Look) -> bool {
    matches!(&output.shown.content, Content::Image(p) if self::look(p) == *look)
}

pub(super) fn develop(
    renderer: &Renderer,
    output: &str,
    picture: &Picture,
    pixels: Option<Arc<RgbaImage>>,
    (fit, size): Target,
) -> Option<Made> {
    let (file, edit, max) = (&picture.file, picture.edit.as_deref(), renderer.max_edge());
    let edge = || needed_edge(file, edit, &picture.place, (fit, size), max);
    let pixels = pixels.map_or_else(|| decode_image(file, edge()).map(Arc::new), Ok);
    let depth = picture.depth.as_ref().map(|d| &*d.map);
    let shown = pixels.map_err(anyhow::Error::from).and_then(|p| {
        let (moving, edit) = (edit.and_then(Edit::moving), edit.filter(|e| e.develops()));
        let frame = edit.map_or(p.dimensions(), |e| e.geometry.frame(p.dimensions()));
        let uv = picture.place.uv(fit, frame, size);
        let image = match edit {
            Some(edit) => renderer.develop(&p, edit, depth, renderer.develop_size(frame, uv, size))?,
            None => renderer.upload_placed(&p, uv, size)?,
        };
        let relief = match (moving, depth) {
            (Some(parallax), Some(map)) if !uv.repeat => {
                Some((renderer.upload_depth(map, edit)?, parallax.clone()))
            }
            _ => None,
        };
        Ok((image, uv, relief))
    });
    shown.inspect_err(|e| tracing::warn!(output, "cannot show the image: {e}")).ok()
}

impl Pictures {
    pub(super) fn shared(
        &self,
        renderer: &Renderer,
        picture: &Picture,
        decoded: Option<(u32, u32)>,
        (fit, size): Target,
    ) -> Option<Made> {
        let (edit, max) = (picture.edit.as_deref(), renderer.max_edge());
        let read = || decoded_size(&picture.file, edit, &picture.place, (fit, size), max);
        let dims = decoded.or_else(read)?;
        let developed = edit.filter(|e| e.develops());
        let frame = developed.map_or(dims, |e| e.geometry.frame(dims));
        let uv = picture.place.uv(fit, frame, size);
        let need = match developed {
            Some(_) => renderer.develop_size(frame, uv, size),
            None => renderer.upload_size(dims, uv, size),
        };
        let found = self.find(&look(picture), need)?;
        let raised = edit.and_then(Edit::moving).is_some() && picture.depth.is_some() && !uv.repeat;
        if raised && found.relief.is_none() {
            return None;
        }
        Some((found.image, uv, found.relief.filter(|_| !uv.repeat)))
    }

    pub(super) fn find(&self, look: &Look, need: (u32, u32)) -> Option<Developed> {
        self.0.iter().find(|d| d.look == *look && serves(d.image.size(), need)).cloned()
    }

    pub(super) fn keep(&mut self, made: Developed, outputs: &mut BTreeMap<String, Output>) {
        let size = made.image.size();
        for output in outputs.values_mut().filter(|o| showing(o, &made.look)) {
            let older =
                output.shown.image.as_ref().filter(|i| !i.same(&made.image) && serves(size, i.size()));
            if older.is_some() {
                let relief = made.relief.clone().filter(|_| output.shown.uv.is_none_or(|uv| !uv.repeat));
                (output.shown.image, output.shown.relief) = (Some(made.image.clone()), relief);
                output.dirty = true;
            }
        }
        self.0.retain(|d| d.look != made.look || !serves(size, d.image.size()));
        self.0.push(made);
    }

    pub(super) fn prune(&mut self, outputs: &BTreeMap<String, Output>) {
        let used = |d: &Developed| {
            let layers = outputs.values().flat_map(Output::layers);
            layers.filter_map(|s| s.image.as_ref()).any(|i| i.same(&d.image))
        };
        self.0.retain(used);
    }
}

#[cfg(test)]
mod tests {
    use super::super::output::{Shown, Sink};
    use super::*;
    use wprs_core::{Fit, Place};

    #[test]
    fn one_developed_picture_serves_every_screen_that_needs_about_its_size() {
        assert!(serves((2000, 1000), (2000, 1000)) && serves((1990, 995), (2000, 1000)));
        assert!(serves((3000, 1500), (2000, 1000)) && !serves((4100, 2050), (2000, 1000)));
        assert!(!serves((1900, 950), (2000, 1000)));
    }

    #[test]
    fn screens_that_show_the_same_picture_share_one_texture() {
        let renderer = match wprs_render::Renderer::shared() {
            Ok(renderer) => renderer,
            Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
        };
        let texture =
            |w: u32, h: u32| renderer.upload_rgba(&vec![0; (w * h * 4) as usize], w, h, w * 4).unwrap();
        let (file, place) = (PathBuf::from("/pictures/hills.jpg"), Place::Fit);
        let picture = Picture { file, modified: None, pixels: None, edit: None, place, depth: None };
        let output = |image: GpuImage| {
            let (content, fit, background) = (Content::Image(picture.clone()), Fit::Fill, [0; 3]);
            let (transition, uv, relief, scene) = (None, None, None, None);
            let shown = Shown { content, fit, background, image: Some(image), transition, uv, relief, scene };
            Output::new((16, 9), shown, Sink::Stream(None), 60)
        };
        let mut outputs =
            BTreeMap::from([("A".into(), output(texture(1000, 500))), ("B".into(), output(texture(64, 32)))]);
        let mut pictures = Pictures::default();
        let made = Developed { look: look(&picture), image: texture(1200, 600), relief: None };
        pictures.keep(made.clone(), &mut outputs);
        assert!(
            outputs["A"].shown.image.as_ref().is_some_and(|i| i.same(&made.image)),
            "the first screen kept its copy"
        );
        assert!(
            !outputs["B"].shown.image.as_ref().unwrap().same(&made.image),
            "a much smaller screen took a large copy"
        );
        assert!(pictures.find(&made.look, (1100, 550)).is_some_and(|d| d.image.same(&made.image)));
        pictures.prune(&outputs);
        assert_eq!(pictures.0.len(), 1);
        outputs.remove("A");
        pictures.prune(&outputs);
        assert!(pictures.0.is_empty(), "a texture nobody shows was kept");
    }

    #[test]
    fn a_second_screen_finds_the_texture_made_from_a_reduced_decode() {
        let renderer = match wprs_render::Renderer::shared() {
            Ok(renderer) => renderer,
            Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
        };
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("wide.png");
        RgbaImage::from_pixel(300, 200, image::Rgba([0, 90, 0, 255])).save(&file).unwrap();
        let picture =
            Picture { file, modified: None, pixels: None, edit: None, place: Place::Fit, depth: None };
        let screen = (Fit::Fill, (192, 108));
        let made = develop(renderer, "A", &picture, None, screen).expect("a texture");
        assert_eq!(made.0.size(), (192, 128), "the picture was not decoded at the size the screen needs");
        let mut pictures = Pictures::default();
        pictures.keep(
            Developed { look: look(&picture), image: made.0.clone(), relief: None },
            &mut BTreeMap::new(),
        );
        let found =
            pictures.shared(renderer, &picture, None, screen).expect("the second screen made its own copy");
        assert!(found.0.same(&made.0));
        let decoded = Some((192, 128));
        assert!(pictures.shared(renderer, &picture, decoded, screen).is_some_and(|f| f.0.same(&made.0)));
        assert!(pictures.shared(renderer, &picture, Some((300, 200)), (Fit::Fit, (600, 400))).is_none());
    }
}
