use crate::effect::Param;
use crate::{Color, Layer, LayerKind, MAX_LAYERS, MAX_PARTICLES, MAX_SIZE, SCHEMA, Scene};
use std::collections::HashSet;

pub(crate) const FAR: f32 = 1e6;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Issue {
    #[error("unsupported schema {0:?}, expected {SCHEMA:?}")]
    Schema(String),
    #[error("{path} is {value}, expected {min}..={max}")]
    Range { path: String, value: f32, min: f32, max: f32 },
    #[error("{path} has {count} entries, at most {max} are allowed")]
    TooMany { path: String, count: usize, max: usize },
    #[error("emitters request {0} particles in total, at most {MAX_PARTICLES} are allowed")]
    ParticleBudget(u64),
    #[error("{0} must not be empty")]
    Empty(String),
    #[error("{0} repeats an id that is already in use")]
    DuplicateId(String),
    #[error("{0} is not one of the options")]
    Choice(String),
    #[error("{0} keyframe times are not strictly increasing")]
    Unsorted(String),
    #[error("{path} targets {target:?}, which is not a bindable layer field of matching type")]
    Binding { path: String, target: String },
}

impl Scene {
    pub fn validate(&self) -> Vec<Issue> {
        let mut c = Check(Vec::new());
        if self.schema != SCHEMA {
            c.0.push(Issue::Schema(self.schema.clone()));
        }
        let (w, h, max) = (self.size.0 as f32, self.size.1 as f32, MAX_SIZE as f32);
        c.params("/size", &[("0", w, 1.0, max), ("1", h, 1.0, max)]);
        c.color("/background", self.background);
        c.count("/layers", self.layers.len(), MAX_LAYERS);
        c.ids("/layers", self.layers.iter().map(|l| &l.id));
        c.ids("/properties", self.properties.iter().map(|p| &p.id));
        self.layers.iter().enumerate().for_each(|(i, l)| c.layer(&format!("/layers/{i}"), l));
        let particles = |l: &Layer| match &l.kind {
            LayerKind::Particles(e) => u64::from(e.max_particles),
            _ => 0,
        };
        let total = self.layers.iter().map(particles).sum();
        if total > u64::from(MAX_PARTICLES) {
            c.0.push(Issue::ParticleBudget(total));
        }
        let json = serde_json::to_value(self).unwrap_or_default();
        self.properties
            .iter()
            .enumerate()
            .for_each(|(i, p)| c.property(&json, &format!("/properties/{i}"), p));
        self.timelines.iter().enumerate().for_each(|(i, t)| c.timeline(&json, &format!("/timelines/{i}"), t));
        c.0
    }
}

pub(crate) struct Check(pub(crate) Vec<Issue>);

impl Check {
    pub(crate) fn params(&mut self, base: &str, params: &[Param]) {
        for &(key, value, min, max) in params {
            if !(min..=max).contains(&value) {
                self.0.push(Issue::Range { path: format!("{base}/{key}"), value, min, max });
            }
        }
    }

    pub(crate) fn color(&mut self, path: &str, Color([r, g, b, a]): Color) {
        self.params(path, &[("0", r, 0.0, 1.0), ("1", g, 0.0, 1.0), ("2", b, 0.0, 1.0), ("3", a, 0.0, 1.0)]);
    }

    pub(crate) fn count(&mut self, path: &str, count: usize, max: usize) {
        if count > max {
            self.0.push(Issue::TooMany { path: path.into(), count, max });
        }
    }

    pub(crate) fn filled(&mut self, path: String, text: &str) {
        if text.trim().is_empty() {
            self.0.push(Issue::Empty(path));
        }
    }

    pub(crate) fn ids<'a>(&mut self, base: &str, ids: impl Iterator<Item = &'a String>) {
        let mut seen = HashSet::new();
        for (i, id) in ids.enumerate() {
            let path = format!("{base}/{i}/id");
            if seen.insert(id) { self.filled(path, id) } else { self.0.push(Issue::DuplicateId(path)) }
        }
    }
}
