use crate::{
    Kind, Layer, MAX_ASSETS, MAX_KEYS, MAX_LAYERS, MAX_PARTICLES, MAX_SIZE, MAX_SYSTEMS, MAX_SYSTEMS2,
    MAX_TIMELINES, Property, SCHEMA, Scene,
};
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Issue {
    #[error("unsupported schema {0:?}, expected {SCHEMA:?}")]
    Schema(String),
    #[error("{path} is {value}, expected {min}..={max}")]
    Range { path: String, value: f32, min: f32, max: f32 },
    #[error("{path} has {count} entries, at most {max} are allowed")]
    TooMany { path: String, count: usize, max: usize },
    #[error("{0} must not be empty")]
    Empty(String),
    #[error("{0} must match [a-z0-9_]{{1,32}}")]
    Id(String),
    #[error("{0} repeats an id that is already in use")]
    DuplicateId(String),
    #[error("{path} names the unknown {what} {id:?}")]
    Unknown { path: String, what: &'static str, id: String },
    #[error("{path} names {id:?}, a property that holds no value")]
    Valueless { path: String, id: String },
    #[error("{0} needs exactly one source: prop, keys, audio, cursor, clock or script")]
    Source(String),
    #[error("{0} does not fit the type of this field")]
    Type(String),
    #[error("{0} keyframe times are not increasing")]
    Unsorted(String),
    #[error("{0} is nested deeper than 8 levels")]
    Depth(String),
    #[error("{0} masks itself through other layers")]
    Cycle(String),
    #[error("{0} must be a texture: effects take texture masks, layer and shape masks go on the layer")]
    EffectMask(String),
    #[error("invalid color {0:?}, expected #rrggbb or #rrggbbaa")]
    Color(String),
    #[error(
        "{0} is a sound layer: only id, type, sound, sounds, mode, pause, stream, play_on, loop, play, volume, pan, fade_in and fade_out apply"
    )]
    Sound(String),
    #[error(
        "{0} is a light layer: only id, type, transform, anchor, visible, parent, inherit, attach and the fields of the light apply"
    )]
    Light(String),
    #[error("the scene has {0} light layers, at most 12 are allowed")]
    Lights(usize),
    #[error("{path} needs {count} variants of its shader, at most 8 are allowed")]
    Variants { path: String, count: usize },
    #[error("{0} needs either sound or sounds")]
    Clips(String),
    #[error("{0} depends on itself through other layers")]
    Circle(String),
    #[error("{path}: {reason}")]
    Effect { path: String, reason: String },
    #[error("{0} may only follow a property or keyframes")]
    SoundSource(String),
    #[error("the scene has {0} sound layers, at most 8 are allowed")]
    Sounds(usize),
    #[error("/properties/volume of a scene with sound must be a slider within 0..=100")]
    Volume,
    #[error("{0} nests particle systems more than two levels deep")]
    Generations(String),
    #[error("{0} names {1:?}, which its model does not have")]
    Missing(String, String),
    #[error("the scene has {0} mesh and model layers, at most 16 are allowed")]
    Figures(usize),
    #[error("the scene has {0} lights that cast shadows, at most 2 are allowed")]
    Shadows(usize),
    #[error("{0} applies inside a group with space 3d only")]
    Flat(String),
    #[error("{0} is locked by its particle system")]
    Locked(String),
    #[error("{0} needs either a duration or both frames and fps")]
    Timing(String),
    #[error("{0} applies to timelines that loop with repeat only")]
    Wrap(String),
}

pub(crate) fn nested<'b>(base: &str, layers: &'b [Layer], out: &mut Vec<(String, &'b str)>) {
    for (i, layer) in layers.iter().enumerate() {
        out.push((format!("{base}/{i}"), layer.id.as_str()));
        if let Kind::Group { layers, .. } = &layer.kind {
            nested(&format!("{base}/{i}/layers"), layers, out);
        }
    }
}

pub(crate) struct Check<'a> {
    pub(crate) issues: Vec<Issue>,
    pub(crate) scene: &'a Scene,
    pub(crate) props: &'a BTreeMap<String, Property>,
    pub(crate) found: BTreeSet<&'static str>,
}

impl Scene {
    pub fn validate(&self, props: &BTreeMap<String, Property>) -> Vec<Issue> {
        self.check(props).issues
    }

    pub fn features(&self) -> BTreeSet<&'static str> {
        self.check(&BTreeMap::new()).found
    }

    fn check<'a>(&'a self, props: &'a BTreeMap<String, Property>) -> Check<'a> {
        let mut c = Check { issues: Vec::new(), scene: self, props, found: BTreeSet::new() };
        if self.schema != SCHEMA {
            c.issues.push(Issue::Schema(self.schema.clone()));
        }
        let [w, h] = self.canvas.size.map(|v| v as f32);
        c.range("/canvas/size/0", w, 1.0, MAX_SIZE as f32);
        c.range("/canvas/size/1", h, 1.0, MAX_SIZE as f32);
        c.color("/canvas/background", self.canvas.background);
        c.range("/clock/max_fps", self.clock.max_fps as f32, 0.0, 240.0);
        c.range("/clock/poster_time", self.clock.poster_time, 0.0, 86_400.0);
        c.uses(self.clock.seed.is_none(), "fresh_seed");
        c.count("/assets", self.assets.len(), MAX_ASSETS);
        for (id, asset) in &self.assets {
            c.id(&format!("/assets/{id}"), id);
            c.filled(&format!("/assets/{id}/path"), &asset.path);
        }
        c.assets();
        c.canvas();
        let layers = self.layers().count();
        c.count("/layers", layers, MAX_LAYERS);
        if layers > MAX_LAYERS {
            return c;
        }
        let mut tree = Vec::new();
        nested("/layers", &self.layers, &mut tree);
        c.ids(tree.into_iter());
        c.layers("/layers", &self.layers, 1);
        c.masks();
        c.parents();
        c.sounds();
        c.lights();
        c.files();
        for (id, prop) in props {
            if let crate::Control::File { default_asset: Some(asset), .. } = &prop.control {
                c.known(
                    &format!("/properties/{id}/default_asset"),
                    "asset",
                    asset,
                    self.assets.contains_key(asset),
                );
            }
        }
        c.count("/particles", self.particles.len(), MAX_SYSTEMS2);
        c.uses(self.particles.len() > MAX_SYSTEMS, "particles2");
        c.lineage();
        c.figures();
        let total: u64 = self.particles.values().map(|s| u64::from(s.capacity)).sum();
        c.range("/particles", total as f32, 0.0, MAX_PARTICLES as f32);
        for (id, system) in &self.particles {
            c.id(&format!("/particles/{id}"), id);
            c.system(&format!("/particles/{id}"), system);
        }
        c.count("/timelines", self.timelines.len(), MAX_TIMELINES);
        c.ids(self.timelines.iter().enumerate().map(|(i, t)| (format!("/timelines/{i}"), t.id.as_str())));
        for (i, tl) in self.timelines.iter().enumerate() {
            let p = format!("/timelines/{i}");
            c.timing(&p, tl);
            c.range(&format!("{p}/speed"), tl.speed, -100.0, 100.0);
            c.range(&format!("{p}/delay"), tl.delay, 0.0, 86_400.0);
            if let Some(trigger) = &tl.trigger {
                c.trigger(&format!("{p}/trigger"), trigger);
            }
            c.events(&p, tl);
        }
        c.scripts();
        c
    }
}

impl Check<'_> {
    pub(crate) fn range(&mut self, path: &str, value: f32, min: f32, max: f32) {
        if !(min..=max).contains(&value) {
            self.issues.push(Issue::Range { path: path.into(), value, min, max });
        }
    }

    pub(crate) fn color(&mut self, path: &str, color: crate::Color) {
        color.0.iter().enumerate().for_each(|(i, c)| self.range(&format!("{path}/{i}"), *c, 0.0, 1.0));
    }

    pub(crate) fn count(&mut self, path: &str, count: usize, max: usize) {
        if count > max {
            self.issues.push(Issue::TooMany { path: path.into(), count, max });
        }
    }

    pub(crate) fn filled(&mut self, path: &str, text: &str) {
        if text.trim().is_empty() {
            self.issues.push(Issue::Empty(path.into()));
        }
    }

    pub(crate) fn id(&mut self, path: &str, id: &str) {
        let valid = (1..=32).contains(&id.len())
            && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if !valid {
            self.issues.push(Issue::Id(path.into()));
        }
    }

    fn ids<'b>(&mut self, ids: impl Iterator<Item = (String, &'b str)>) {
        let mut seen = HashSet::new();
        for (at, id) in ids {
            let path = format!("{at}/id");
            self.id(&path, id);
            if !seen.insert(id) {
                self.issues.push(Issue::DuplicateId(path));
            }
        }
    }

    pub(crate) fn property(&mut self, path: &str, id: &str, button: bool) {
        match self.props.get(id) {
            None => self.known(path, "property", id, false),
            Some(p) if p.holds_value() || (button && p.kind() == "button") => {}
            Some(_) => self.issues.push(Issue::Valueless { path: path.into(), id: id.into() }),
        }
    }

    pub(crate) fn known(&mut self, path: &str, what: &'static str, id: &str, exists: bool) {
        if !exists {
            self.issues.push(Issue::Unknown { path: path.into(), what, id: id.into() });
        }
    }

    pub(crate) fn keys_sorted(&mut self, path: &str, times: impl Iterator<Item = f32>) {
        let times: Vec<f32> = times.collect();
        self.count(path, times.len(), MAX_KEYS);
        if times.windows(2).any(|w| w[0] > w[1]) || times.iter().any(|t| !t.is_finite() || *t < 0.0) {
            self.issues.push(Issue::Unsorted(path.into()));
        }
    }

    pub(crate) fn trigger(&mut self, path: &str, trigger: &str) {
        match trigger.split_once(':') {
            None if matches!(trigger, "start" | "beat") => {}
            Some(("prop", id)) => self.property(path, id, true),
            Some(("event", name)) => {
                let named = self.fired(name);
                self.known(path, "event", name, named);
                self.uses(true, "timeline_events");
            }
            Some(("click", id)) => self.pointed(path, "layer that takes clicks", id),
            Some(("enter" | "leave", id)) => self.pointed(path, "layer that takes the pointer", id),
            Some(("media", what)) => self.heard(path, what),
            _ => self.known(path, "trigger", trigger, false),
        }
    }

    pub(crate) fn uses(&mut self, on: bool, feature: &'static str) {
        if on {
            self.found.insert(feature);
        }
    }
}
