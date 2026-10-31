use super::super::output::{Output, Shown, Sink};
use super::super::pictures::{self, Developed, Made};
use super::super::scene::SceneState;
use super::super::surface::Screen;
use super::super::{Content, Picture, Settings, Spec};
use super::State;
use image::RgbaImage;
use std::sync::Arc;
use std::time::Instant;
use wprs_core::Fit;

impl State {
    pub(super) fn configure(&mut self, name: String, size: (u32, u32), mut spec: Spec) {
        let slot = spec.content.video().and_then(|reel| self.players.get_mut(reel));
        slot.into_iter().for_each(|slot| slot.revive(Instant::now()));
        self.outputs.get_mut(&name).into_iter().for_each(|o| o.refresh = spec.refresh);
        let resume = self.outputs.get(&name).and_then(|o| match (&o.shown.content, &spec.content) {
            (Content::Scene(old), Content::Scene(new))
                if old.pack.path == new.pack.path && new.hold.is_none() =>
            {
                o.shown.scene.as_ref().map(SceneState::time)
            }
            _ => None,
        });
        let mut current = self.outputs.get_mut(&name).filter(|o| o.size == size);
        if current.as_ref().is_some_and(|o| o.shown.matches(&spec)) {
            return;
        }
        if let Some(o) = current.as_deref_mut()
            && let (Content::Scene(old), Content::Scene(new)) = (&o.shown.content, &spec.content)
            && Arc::ptr_eq(&old.pack, &new.pack)
            && (o.shown.fit, o.shown.background) == (spec.fit, spec.background)
            && let Some(scene) = o.shown.scene.as_mut()
        {
            o.dirty |= scene.update(new.clone());
            scene.link(&mut self.scripts);
            super::super::feed::declare(&name, Some(scene));
            o.shown.content = spec.content;
            return;
        }
        let same = |o: &&mut Output| o.shown.fit == spec.fit && o.shown.content.same_look(&spec.content);
        let kept = current
            .filter(same)
            .and_then(|o| Some((o.shown.image.take()?, o.shown.uv?, o.shown.relief.take())));
        let (image, uv, relief) = match &mut spec.content {
            Content::Image(picture) => {
                let pixels = picture.pixels.take();
                kept.or_else(|| self.upload(&name, picture, pixels, spec.fit, size))
            }
            _ => kept,
        }
        .map_or((None, None, None), |(image, uv, relief)| (Some(image), Some(uv), relief));
        let (content, fit, background, transition, refresh) =
            (spec.content, spec.fit, spec.background, spec.transition, spec.refresh);
        let scene = match &content {
            Content::Scene(s) => {
                let mut scene = SceneState::new(s.clone(), self.power);
                scene.link(&mut self.scripts);
                scene.calm(self.motion.calm);
                scene.guard(self.settings.flash_guard);
                resume.into_iter().for_each(|time| scene.resume(time));
                Some(scene)
            }
            _ => None,
        };
        super::super::feed::declare(&name, scene.as_ref());
        let shown = Shown { content, fit, background, image, uv, transition, relief, scene };
        let transition = !transition.unwrap_or(self.settings.transition).is_zero();
        match self.outputs.get_mut(&name) {
            Some(output) => output.show(shown, size, transition),
            None => {
                let sink =
                    self.windows.remove(&name).map_or(Sink::Stream(None), |w| Sink::Surface(Screen::new(w)));
                self.outputs.insert(name.clone(), Output::new(size, shown, sink, refresh));
            }
        }
        self.pictures.prune(&self.outputs);
        self.follow(&name);
    }

    fn upload(
        &mut self,
        output: &str,
        picture: &Picture,
        pixels: Option<Arc<RgbaImage>>,
        fit: Fit,
        size: (u32, u32),
    ) -> Option<Made> {
        let decoded = pixels.as_deref().map(RgbaImage::dimensions);
        if let Some(shared) = self.pictures.shared(self.renderer, picture, decoded, (fit, size)) {
            return Some(shared);
        }
        let made = pictures::develop(self.renderer, output, picture, pixels, (fit, size))?;
        let developed =
            Developed { look: pictures::look(picture), image: made.0.clone(), relief: made.2.clone() };
        self.pictures.keep(developed, &mut self.outputs);
        Some(made)
    }

    pub(super) fn apply_settings(&mut self, settings: Settings) {
        let old = std::mem::replace(&mut self.settings, settings);
        self.power.max_fps = settings.max_fps;
        self.outputs.values_mut().for_each(|o| o.guard(settings.flash_guard));
        if old.software_parallax != settings.software_parallax && self.motion.software {
            self.restill();
        }
        self.hush();
        let mut players = self.players.values().filter(|s| s.volume > 0).filter_map(|s| s.player.as_ref());
        if !settings.muted && settings.volume > 0.0 && players.any(|p| !p.audio_enabled()) {
            self.players.retain(|_, slot| slot.volume == 0);
        }
    }

    pub(in crate::compositor) fn hush(&self) {
        let heard = (self.settings, self.locked);
        self.players.values().for_each(|slot| slot.hush(heard));
    }
}
