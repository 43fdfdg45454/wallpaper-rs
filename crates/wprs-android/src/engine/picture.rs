use super::fade::{Fading, Gone};
use super::{Engine, FRAMES};
use crate::framing;
use crate::gpu::{self, Key};
use crate::power::{Power, delay_ms};
use crate::relief::{self, Look as Seen, layer};
use crate::settings;
use crate::stage::Stage;
use crate::{choice, store};
use std::sync::atomic::Ordering::Relaxed;
use wprs_render::RenderError;

impl Engine {
    pub(super) fn frame(&mut self, nanos: i64, fps: u32, power: Power) -> Result<(u32, i64), RenderError> {
        if self.target.as_ref().is_none_or(|(gpu, _)| !gpu.current()) {
            (self.target, self.fading, self.dirty) = (None, None, true);
            let opened = gpu::open(&self.window, self.size)?;
            if std::mem::replace(&mut self.checked.0, 0) != opened.0.epoch {
                (self.image, self.raised, self.checked) = (None, None, (0, 0, (0, 0)));
            } else {
                self.checked = (opened.0.epoch, 0, (0, 0));
            }
            self.target = Some(opened);
        }
        let epoch = self.target.as_ref().map_or(0, |(gpu, _)| gpu.epoch);
        let check = (epoch, store::generation(), self.size);
        if self.checked != check {
            let old = std::mem::replace(&mut self.checked, check).0;
            let (size, shown) = (self.size, choice::shown_on(self.preview, self.lock));
            if shown.as_ref().is_some_and(|shown| shown.video) {
                (self.image, self.framed, self.crop) = (None, None, None);
                return self.compose(nanos, fps, power);
            }
            self.stage(shown.as_ref());
            if old != epoch {
                self.stage.iter_mut().for_each(Stage::drop_view);
            }
            let Some((gpu, _)) = self.target.as_ref() else { return Ok((0, 0)) };
            let before = (self.look.fit, self.crop);
            self.look = settings::look();
            let shown = shown.filter(|shown| !shown.scene);
            let framed =
                shown.as_ref().map(|s| framing::framed(&s.uid, &s.path, self.look.fit, self.preview));
            self.look.fit = framed.as_ref().map_or(self.look.fit, |framed| framed.fit);
            let geometry = framed.as_ref().and_then(|f| f.develop.as_ref()).map(|edit| &edit.geometry);
            let raise = shown.as_ref().map(|s| (&*s.uid, &*s.path));
            self.dirty |= relief::raise(&mut self.raised, gpu, raise, geometry);
            let fit = self.look.fit;
            let wanted = shown.zip(framed).map(|(shown, framed)| {
                let look = format!(
                    "{:?}{:?}{:?}",
                    framed.develop,
                    framed.map,
                    framed.crop.map(|[.., w, h]| w.min(h).log2().floor())
                );
                (Key { epoch: gpu.epoch, item: shown.uid, fit, size, look }, shown.path, framed)
            });
            self.framed = wanted.as_ref().map(|(key, _, framed)| (key.item.clone(), framed.clone()));
            if self.image.as_ref().map(|(key, _)| key) != wanted.as_ref().map(|(key, ..)| key) {
                let loaded = wanted
                    .and_then(|(key, path, framed)| Some((key.clone(), gpu.texture(key, &path, &framed)?)));
                let previous = std::mem::replace(&mut self.image, loaded);
                let scene = self.stage.is_some();
                let other = |old: &Key| self.image.as_ref().map_or(scene, |(new, _)| new.item != old.item);
                let fade =
                    previous.filter(|(old, _)| (old.epoch, old.size) == (gpu.epoch, size) && other(old));
                let fades = fps > 0 && (self.image.is_some() || scene) && !self.look.fade.is_zero();
                let fade = fade.filter(|_| fades && self.fading.is_none());
                self.fading =
                    fade.map(|(_, old)| Fading::new(Gone::Picture(old, before))).or(self.fading.take());
                self.dirty = true;
                gpu::trim();
            }
        }
        if self.fading.is_some() || self.film.is_some() {
            return self.compose(nanos, fps, power);
        }
        if let Some(next) = self.shoot(nanos, fps, power)? {
            return Ok(next);
        }
        let Some((gpu, surface)) = self.target.as_mut() else { return Ok((0, 0)) };
        if power.moving() && self.offset != self.shown {
            (self.shown, self.dirty) = (self.offset, true);
        }
        let crop = self.framed.as_ref().and_then(|(uid, framed)| framed.shown(uid, self.preview));
        if crop != self.crop {
            (self.crop, self.dirty) = (crop, true);
        }
        let raised =
            self.raised.as_mut().map(|r| r.step(self.offset, (nanos, fps), power.moving(), self.size));
        let (moved, tilting) = raised.unwrap_or_default();
        self.dirty |= moved;
        let animating = |on: bool| if on { (fps, delay_ms(fps)) } else { (0, 0) };
        if !self.dirty {
            return Ok(animating(tilting));
        }
        let (fit, background, pan, size) = (self.look.fit, self.look.background, self.shown, self.size);
        let look = Seen { fit, background, pan, crop: self.crop };
        let current = self.image.as_ref().map(|(_, image)| image.as_ref());
        let raised = self.raised.as_ref().map(|raised| (raised, power.charging));
        let layers = [layer(current, 1.0, &look, size, raised)];
        self.dirty = !gpu.renderer.present_layers(surface, size, &layers, || {})?;
        if !self.dirty {
            self.presented = nanos;
            FRAMES.fetch_add(1, Relaxed);
        }
        Ok(animating(tilting))
    }
}
