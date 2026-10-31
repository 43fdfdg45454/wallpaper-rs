use super::{Engine, FRAMES};
use crate::Error;
use crate::choice::{self, Shown};
use crate::pack;
use crate::power::{self, Power};
use crate::stage::Stage;
use ndk::native_window::NativeWindow;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use wprs_core::Fit;
use wprs_render::{Layer, RenderError};

impl Engine {
    pub(super) fn stage(&mut self, shown: Option<&Shown>) {
        match shown.filter(|shown| shown.scene).map(|shown| (&shown.uid, pack::load(&shown.path))) {
            Some((uid, Ok(pack))) => match self.stage.as_mut() {
                Some(stage) if stage.uid == *uid && Arc::ptr_eq(&stage.pack, &pack) => stage.refresh(),
                _ => self.stage = Some(Stage::new(uid.clone(), pack)),
            },
            Some((_, Err(e))) => {
                log::warn!("cannot show the scene: {e}");
                self.stage = None;
            }
            None => self.stage = None,
        }
    }

    pub(crate) fn clips(&mut self) -> Option<Vec<String>> {
        let shown = choice::shown_on(self.preview, self.lock).filter(|shown| !shown.video);
        self.stage(shown.as_ref());
        let videos = &self.stage.as_ref()?.pack.videos;
        Some(videos.iter().filter_map(|(_, path)| path.to_str().map(String::from)).collect())
    }

    pub(crate) fn clip(&mut self, path: &Path, open: bool) -> Result<Option<NativeWindow>, Error> {
        if !open {
            self.clips.close(path);
            return Ok(None);
        }
        let wanted = self.stage.as_ref().is_some_and(|s| s.pack.videos.iter().any(|(_, p)| p == path));
        wanted.then(|| self.clips.open(path)).transpose()
    }

    pub(crate) fn inputs(&self) -> i32 {
        let [audio, touch] = self.stage.as_ref().map_or([false; 2], Stage::wants);
        let (moving, plays) = (Power::now().moving(), self.stage.as_ref().is_some_and(Stage::plays));
        i32::from(audio && moving && crate::power::hears())
            | (i32::from(touch) << 1)
            | (i32::from(plays && moving) << 2)
    }

    pub(crate) fn touch(&mut self, x: f32, y: f32, down: bool) -> bool {
        let at = [x / self.size.0 as f32, y / self.size.1 as f32].map(|v| v.clamp(0.0, 1.0));
        self.stage.as_mut().is_some_and(|stage| stage.touch(at, down))
    }

    pub(crate) fn visible(&mut self, shown: bool) {
        if !shown {
            self.forget_old_gpu();
            self.hint(0);
            self.stage.iter_mut().for_each(Stage::hide);
        }
    }

    pub(super) fn shoot(
        &mut self,
        nanos: i64,
        fps: u32,
        power: Power,
    ) -> Result<Option<(u32, i64)>, RenderError> {
        let (Some(stage), Some((gpu, surface))) = (self.stage.as_mut(), self.target.as_mut()) else {
            return Ok(None);
        };
        let tier = power::tier(gpu.renderer.software(), gpu.renderer.compute()).min(gpu.renderer.tier());
        let pack = Arc::clone(&stage.pack);
        let videos = self.clips.frames(&gpu.renderer, gpu.epoch, &pack.videos);
        let shot =
            stage.render(&gpu.renderer, (self.size, nanos, fps), (power, tier), &videos, self.dirty)?;
        let (hint, delay) = (shot.fps, shot.delay);
        if let Some(image) = shot.image {
            let shown = Layer {
                image: Some(image),
                fit: Fit::Stretch,
                background: [0; 3],
                opacity: 1.0,
                uv: None,
                relief: None,
            };
            self.dirty = !gpu.renderer.present_layers(surface, self.size, &[shown], || {})?;
            if !self.dirty {
                self.presented = nanos;
                FRAMES.fetch_add(1, Relaxed);
            }
        }
        if delay == 0 {
            stage.rest();
        }
        Ok(Some((hint, delay)))
    }
}
