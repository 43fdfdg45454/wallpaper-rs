use crate::validate::{Check, Issue};
use crate::value::Value;
use crate::{Descriptor, EffectUse, MAX_EFFECT_FILES, MAX_EVENTS, MAX_VARIANTS, Timeline};

impl Check<'_> {
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
