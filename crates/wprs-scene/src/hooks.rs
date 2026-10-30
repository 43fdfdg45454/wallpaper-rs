use crate::media::{MEDIA_TRIGGERS, Reading};
use crate::probe::Probe;
use crate::scripting::{
    MAX_POOL_LAYERS, MAX_POOL_VERTICES, MAX_POOLS, MAX_SCRIPT_PARAMS, MAX_SCRIPTS2, MAX_TEMPLATES, ScriptApi,
};
use crate::validate::{Check, Issue};
use crate::value::{Binding, Param};
use crate::{Kind, Layer, MAX_SCRIPTS, Solid};

const LIFECYCLE: [&str; 7] =
    ["init", "update", "destroy", "resize", "properties", "settings", "timelineEvent"];
const BASIC: [ScriptApi; 4] = [ScriptApi::Audio, ScriptApi::Pointer, ScriptApi::Clicks, ScriptApi::Particles];

fn identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
        && name.len() <= 64
}

impl Check<'_> {
    pub(crate) fn scripts(&mut self) {
        let scene = self.scene;
        self.count("/scripts", scene.scripts.len(), MAX_SCRIPTS2);
        self.uses(scene.scripts.len() > MAX_SCRIPTS, "scripts2");
        let solid = scene.layers().any(|l| !l.solid.is_off() || l.rig().is_some_and(|r| !r.hit.is_empty()));
        for (i, s) in scene.scripts.iter().enumerate() {
            let p = format!("/scripts/{i}");
            self.filled(&format!("{p}/path"), &s.path);
            if scene.scripts[..i].iter().any(|o| o.path == s.path) {
                self.issues.push(Issue::DuplicateId(format!("{p}/path")));
            }
            if s.events.iter().enumerate().any(|(k, e)| s.events[..k].contains(e)) {
                self.issues.push(Issue::DuplicateId(format!("{p}/events")));
            }
            if s.uses.iter().enumerate().any(|(k, e)| s.uses[..k].contains(e)) {
                self.issues.push(Issue::DuplicateId(format!("{p}/uses")));
            }
            let newer = s.events.iter().any(|e| e.newer()) || s.uses.iter().any(|u| !BASIC.contains(u));
            self.uses(newer || !s.params.is_empty(), "scripts2");
            self.uses(s.wants(ScriptApi::Particles), "particles2");
            self.uses(s.wants(ScriptApi::Storage), "script_storage");
            self.uses(s.wants(ScriptApi::Layers), "script_layers");
            self.uses(s.wants(ScriptApi::Geometry), "script_geometry");
            self.uses(s.wants(ScriptApi::Sounds), "script_sounds");
            self.uses(s.wants(ScriptApi::Animations), "script_animations");
            self.uses(s.wants(ScriptApi::Camera), "script_camera");
            self.uses(s.wants(ScriptApi::Shortcuts), "script_shortcuts");
            self.uses(s.wants(ScriptApi::Hierarchy), "script_hierarchy");
            self.uses(s.wants(ScriptApi::Media) || s.events.iter().any(|e| e.media()), "media_info");
            let touches = s.events.iter().any(|e| e.pointer() || e.clicks());
            self.uses(touches, "layer_events");
            if touches && !solid {
                self.issues.push(Issue::Type(format!("{p}/events")));
            }
            if s.wants(ScriptApi::Geometry) && scene.pools.is_empty() {
                self.issues.push(Issue::Type(format!("{p}/uses")));
            }
            self.count(&format!("{p}/params"), s.params.len(), MAX_SCRIPT_PARAMS);
            for (id, d) in &s.params {
                let q = format!("{p}/params/{id}");
                self.id(&q, id);
                if !d.accepts(&d.default) {
                    self.issues.push(Issue::Type(format!("{q}/default")));
                }
                let (lo, hi) = (d.min.unwrap_or(-1e6), d.max.unwrap_or(1e6));
                self.range(&format!("{q}/max"), hi, lo, 1e6);
                if let Param::Number(n) = d.default {
                    self.range(&format!("{q}/default"), n, lo, hi);
                }
            }
        }
        self.count("/templates", scene.templates.len(), MAX_TEMPLATES);
        for (id, layer) in &scene.templates {
            let p = format!("/templates/{id}");
            self.uses(true, "script_layers");
            self.id(&p, id);
            self.id(&format!("{p}/id"), &layer.id);
            if scene.layer(id).is_some() {
                self.issues.push(Issue::DuplicateId(p.clone()));
            }
            let mut inner = vec![layer];
            while let Some(l) = inner.pop() {
                inner.extend(l.children());
                let copied = matches!(
                    l.kind,
                    Kind::Group { .. }
                        | Kind::Image { .. }
                        | Kind::Text(_)
                        | Kind::Shape { .. }
                        | Kind::Solid { .. }
                        | Kind::Spectrum(_)
                );
                if !copied {
                    self.issues.push(Issue::Type(format!("{p}/type")));
                }
            }
            self.layer(&p, layer, 1);
        }
        self.count("/pools", scene.pools.len(), MAX_POOLS);
        for (id, pool) in &scene.pools {
            let p = format!("/pools/{id}");
            self.uses(true, "script_geometry");
            self.id(&p, id);
            self.range(&format!("{p}/max"), pool.max as f32, 1.0, MAX_POOL_LAYERS as f32);
            self.range(&format!("{p}/max_vertices"), pool.max_vertices as f32, 3.0, MAX_POOL_VERTICES as f32);
            pool.texture.iter().for_each(|t| self.picture(&format!("{p}/texture"), t));
        }
    }

    pub(crate) fn hooks(&mut self, p: &str, layer: &Layer) {
        if let Solid::Hit { alpha } = layer.solid {
            self.uses(true, "layer_events");
            self.range(&format!("{p}/solid/alpha"), alpha, 0.0, 1.0);
        }
        for (i, e) in layer.effects.iter().enumerate() {
            let Some(name) = &e.name else { continue };
            self.uses(true, "scripts2");
            self.id(&format!("{p}/effects/{i}/name"), name);
            if layer.effects[..i].iter().any(|o| o.name.as_ref() == Some(name)) {
                self.issues.push(Issue::DuplicateId(format!("{p}/effects/{i}/name")));
            }
        }
        if let Kind::Adjustment { rgb: true, .. } = layer.kind {
            self.uses(true, "rgb");
        }
    }

    pub(crate) fn called<T: Probe>(&mut self, path: &str, b: &Binding<T>) {
        if let Some(m) = b.media {
            self.uses(true, "media_info");
            let fits = match m.reading() {
                Reading::Words => T::TEXT,
                Reading::Number => T::SCALAR,
                Reading::Flag => T::FLAG || T::SCALAR,
                Reading::Color => T::COLOR,
            };
            if !fits {
                self.issues.push(Issue::Type(format!("{path}/media")));
            }
        }
        if b.call.is_none() && b.params.is_none() {
            return;
        }
        self.uses(true, "scripts2");
        if b.call.is_none() {
            self.issues.push(Issue::Type(format!("{path}/fn")));
        }
        let Some(script) = &b.script else {
            return self.issues.push(Issue::Type(path.into()));
        };
        if let Some(name) = &b.call
            && (!identifier(name) || LIFECYCLE.contains(&name.as_str()))
        {
            self.issues.push(Issue::Type(format!("{path}/fn")));
        }
        let declared = self.scene.scripts.iter().find(|s| &s.path == script).map(|s| &s.params);
        for (id, v) in b.params.iter().flatten() {
            let q = format!("{path}/params/{id}");
            match declared.and_then(|d| d.get(id)) {
                None => self.known(&q, "script parameter", id, false),
                Some(d) if !d.accepts(v) => self.issues.push(Issue::Type(q)),
                Some(d) => {
                    if let Param::Number(n) = v {
                        self.range(&q, *n, d.min.unwrap_or(-1e6), d.max.unwrap_or(1e6));
                    }
                }
            }
        }
    }

    pub(crate) fn pointed(&mut self, path: &str, kind: &'static str, id: &str) {
        self.uses(true, "layer_events");
        let scene = self.scene;
        let solid =
            scene.layer(id).is_some_and(|l| !l.solid.is_off() || l.rig().is_some_and(|r| !r.hit.is_empty()));
        self.known(path, kind, id, solid);
    }

    pub(crate) fn heard(&mut self, path: &str, what: &str) {
        self.uses(true, "media_info");
        self.known(path, "media trigger", what, MEDIA_TRIGGERS.contains(&what));
    }

    pub(crate) fn fired(&self, name: &str) -> bool {
        let scene = self.scene;
        let clips = scene.layers().filter_map(Layer::rig).flat_map(|r| &r.clips).flat_map(|c| &c.events);
        scene.timelines.iter().flat_map(|t| &t.events).chain(clips).any(|e| e.name == name)
    }
}
