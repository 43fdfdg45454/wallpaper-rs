use crate::validate::{Check, Issue};
use crate::value::Value;
use crate::{Descriptor, EffectUse, MAX_EFFECT_FILES, MAX_EVENTS, MAX_FRAMES, MAX_VARIANTS, Timeline};

pub(crate) const IMAGES: [&str; 8] = ["png", "jpg", "jpeg", "webp", "gif", "bmp", "avif", "ktx2"];
pub(crate) const VIDEOS: [&str; 3] = ["webm", "mp4", "mkv"];

impl Check<'_> {
    pub(crate) fn canvas(&mut self) {
        let c = &self.scene.canvas;
        if let Some(f) = &c.parallax {
            self.uses(true, "parallax2");
            self.range("/canvas/parallax/amount", f.amount, 0.0, 4.0);
            self.range("/canvas/parallax/delay", f.delay, 0.0, 2.0);
            self.range("/canvas/parallax/pointer", f.pointer, 0.0, 1.0);
        }
        if let Some(cam) = &c.camera {
            self.uses(true, "camera");
            cam.center.iter().for_each(|v| self.value("/canvas/camera/center", v));
            self.number("/canvas/camera/zoom", &cam.zoom, 0.1, 10.0);
            self.number("/canvas/camera/rotation", &cam.rotation, -360.0, 360.0);
            if let Some(s) = &cam.shake {
                self.value("/canvas/camera/shake/enabled", &s.enabled);
                self.number("/canvas/camera/shake/amplitude", &s.amplitude, 0.0, 1000.0);
                self.number("/canvas/camera/shake/roughness", &s.roughness, 0.0, 1.0);
                self.number("/canvas/camera/shake/speed", &s.speed, 0.0, 60.0);
            }
        }
        if let Some(b) = &c.bloom {
            self.uses(true, "bloom");
            self.number("/canvas/bloom/threshold", &b.threshold, 0.0, 4.0);
            self.number("/canvas/bloom/strength", &b.strength, 0.0, 8.0);
            self.number("/canvas/bloom/radius", &b.radius, 0.0, 128.0);
            self.value("/canvas/bloom/tint", &b.tint);
            if let Some(h) = b.hdr {
                self.range("/canvas/bloom/hdr/threshold", h.threshold, 0.0, 16.0);
                self.range("/canvas/bloom/hdr/strength", h.strength, 0.0, 8.0);
            }
        }
        self.uses(c.hdr || c.tonemap.is_some(), "hdr");
        if c.tonemap.is_some() && !c.hdr {
            self.issues.push(Issue::Type("/canvas/tonemap".into()));
        }
        self.uses(!c.clear, "keep_frame");
        if let Some(d) = c.perspective {
            self.uses(true, "tilt");
            self.range("/canvas/perspective", d, 100.0, 1e5);
        }
        for (name, color) in [("ambient", c.ambient), ("sky", c.sky)] {
            let Some(color) = color else { continue };
            self.uses(true, "ambient_light");
            self.color(&format!("/canvas/{name}"), color);
        }
        if let Some(step) = self.scene.clock.step {
            self.uses(true, "clock_values");
            self.range("/clock/step", step, 1.0, 3600.0);
        }
        if let Some(a) = &self.scene.audio {
            self.uses(true, "audio_response");
            self.range("/audio/attack", a.attack, 0.001, 2.0);
            self.range("/audio/release", a.release, 0.01, 10.0);
        }
    }

    pub(crate) fn assets(&mut self) {
        for (id, a) in &self.scene.assets {
            let (p, ext) = (format!("/assets/{id}"), a.extension());
            let picture = IMAGES.contains(&ext.as_str());
            self.uses(a.sampled(), "texture_options");
            self.uses(a.collection(), "fonts2");
            self.uses(a.moves(), "animated_textures");
            let face = a.face.is_some() && !["ttc", "otc"].contains(&ext.as_str());
            let sheet = a.atlas.is_some() || !a.rects.is_empty() || a.animated;
            let set =
                |fields: [(&'static str, bool); 3]| fields.into_iter().find(|(_, on)| *on).map(|(n, _)| n);
            let sampled = set([
                ("filter", a.filter.is_some()),
                ("wrap", a.wrap.is_some()),
                ("mipmaps", a.mipmaps.is_some()),
            ]);
            let timed = set([
                ("frames", a.frames.is_some()),
                ("fps", a.fps.is_some()),
                ("durations", !a.durations.is_empty()),
            ]);
            let wrong = [
                (sampled.unwrap_or_default(), !picture),
                ("face", face),
                ("animated", a.animated && !["gif", "webp"].contains(&ext.as_str())),
                ("rects", !a.rects.is_empty() && (a.atlas.is_some() || !picture)),
                (timed.unwrap_or_default(), !sheet || !picture),
                ("durations", a.fps.is_some() && !a.durations.is_empty()),
            ];
            let wrong = wrong.into_iter().filter(|(name, _)| !name.is_empty());
            for (name, _) in wrong.filter(|(_, w)| *w) {
                self.issues.push(Issue::Type(format!("{p}/{name}")));
            }
            if a.media.is_some() {
                self.uses(true, "media_info");
                if !picture || a.moves() {
                    self.issues.push(Issue::Type(format!("{p}/media")));
                }
            }
            a.face.iter().for_each(|f| self.range(&format!("{p}/face"), *f as f32, 0.0, 255.0));
            self.count(&format!("{p}/rects"), a.rects.len(), MAX_FRAMES as usize);
            for (i, r) in a.rects.iter().enumerate() {
                self.range(&format!("{p}/rects/{i}/2"), r[2] as f32, 1.0, 16_384.0);
                self.range(&format!("{p}/rects/{i}/3"), r[3] as f32, 1.0, 16_384.0);
            }
            let cells = if a.animated { MAX_FRAMES } else { a.cells().min(MAX_FRAMES) };
            a.frames.iter().for_each(|n| self.range(&format!("{p}/frames"), *n as f32, 1.0, cells as f32));
            a.fps.iter().for_each(|f| self.range(&format!("{p}/fps"), *f, 0.1, 120.0));
            if !a.durations.is_empty() {
                let n = a.frames.unwrap_or(cells) as f32;
                self.range(&format!("{p}/durations"), a.durations.len() as f32, n, n);
                for (i, d) in a.durations.iter().enumerate() {
                    self.range(&format!("{p}/durations/{i}"), *d, 0.001, 60.0);
                }
            }
        }
    }

    pub(crate) fn moving(&mut self, named: &Value<String>) {
        let ids: Vec<&String> = match named {
            Value::Fixed(id) => vec![id],
            Value::Bound(b) => b.map.iter().flat_map(|m| m.values()).collect(),
        };
        let assets = ids.into_iter().filter_map(|id| self.scene.assets.get(id));
        let video = assets.into_iter().any(|a| VIDEOS.contains(&a.extension().as_str()));
        self.uses(video, "video_textures");
    }

    pub(crate) fn files(&mut self) {
        let scene = self.scene;
        self.count("/effects", scene.effects.len(), MAX_EFFECT_FILES);
        for (id, f) in &scene.effects {
            let p = format!("/effects/{id}");
            self.uses(true, "custom_effects");
            self.id(&p, id);
            if crate::reserved(id) {
                self.issues.push(Issue::DuplicateId(p.clone()));
            }
            for (name, path, ext) in [("descriptor", &f.descriptor, ".json"), ("wgsl", &f.wgsl, ".wgsl")] {
                if !path.to_ascii_lowercase().ends_with(ext) {
                    self.issues.push(Issue::Type(format!("{p}/{name}")));
                }
            }
        }
        for d in scene.loaded.values() {
            self.uses(!d.options.is_empty(), "effect_options");
            self.uses(d.passes.iter().any(|p| p.vertex.is_some()), "vertex_effects");
            self.uses(d.newest(), "custom_effects2");
        }
    }

    pub(crate) fn timing(&mut self, p: &str, tl: &Timeline) {
        let counted = tl.frames.is_some() || tl.fps.is_some();
        self.uses(counted, "timeline_frames");
        self.uses(tl.wrap, "timeline_wrap");
        if counted && (tl.frames.is_none() || tl.fps.is_none() || tl.duration != 0.0) {
            self.issues.push(Issue::Timing(p.into()));
        }
        if let Some(fps) = tl.fps {
            self.range(&format!("{p}/fps"), fps, 1.0, 240.0);
        }
        let place = if counted { "frames" } else { "duration" };
        self.range(&format!("{p}/{place}"), tl.length(), 1e-3, 86_400.0);
        if tl.wrap && tl.repeat != crate::Loop::Repeat {
            self.issues.push(Issue::Wrap(format!("{p}/wrap")));
        }
    }

    pub(crate) fn events(&mut self, p: &str, tl: &Timeline) {
        self.count(&format!("{p}/events"), tl.events.len(), MAX_EVENTS);
        self.uses(!tl.events.is_empty(), "timeline_events");
        for (i, e) in tl.events.iter().enumerate() {
            self.range(&format!("{p}/events/{i}/t"), e.t, 0.0, tl.length());
            self.id(&format!("{p}/events/{i}/name"), &e.name);
        }
    }

    pub(crate) fn variants(&mut self, e: &str, d: &Descriptor, effect: &EffectUse) {
        for o in &d.options {
            let path = format!("{e}/params/{}", o.id);
            let Some(value) = effect.params.get(&o.id) else { continue };
            self.value(&path, value);
            match value {
                Value::Fixed(v) if o.values.contains(v) => {}
                Value::Bound(b) if b.prop.is_some() => {
                    if b.map.as_ref().is_some_and(|map| !map.values().all(|v| o.values.contains(v))) {
                        self.issues.push(Issue::Type(format!("{path}/map")));
                    }
                }
                _ => self.issues.push(Issue::Type(path)),
            }
        }
        let count = d.combinations(effect);
        if count > MAX_VARIANTS {
            self.issues.push(Issue::Variants { path: format!("{e}/params"), count });
        }
    }
}
