use crate::SCHEMA;
use wprs_core::message::{Message, said};

#[derive(Debug, Clone, PartialEq)]
pub enum Issue {
    Schema(String),
    Range { path: String, value: f32, min: f32, max: f32 },
    TooMany { path: String, count: usize, max: usize },
    Empty(String),
    Id(String),
    DuplicateId(String),
    Unknown { path: String, what: &'static str, id: String },
    Valueless { path: String, id: String },
    Source(String),
    Type(String),
    Unsorted(String),
    Depth(String),
    Cycle(String),
    Color(String),
    Sound(String),
    Light(String),
    Lights(usize),
    Variants { path: String, count: usize },
    Clips(String),
    Circle(String),
    Effect { path: String, reason: String },
    SoundSource(String),
    Sounds(usize),
    Volume,
    Generations(String),
    Missing(String, String),
    Figures(usize),
    Shadows(usize),
    Flat(String),
    Locked(String),
    Timing(String),
    Wrap(String),
}

impl Issue {
    pub fn message(&self) -> Message {
        match self {
            Issue::Schema(s) => {
                said("scene.schema", "unsupported schema \"%1\", expected \"%2\"", &[s, &SCHEMA])
            }
            Issue::Range { path, value, min, max } => {
                said("scene.range", "%1 is %2, expected %3..=%4", &[path, value, min, max])
            }
            Issue::TooMany { path, count, max } => {
                said("scene.too_many", "%1 has %2 entries, at most %3 are allowed", &[path, count, max])
            }
            Issue::Empty(p) => said("scene.empty", "%1 must not be empty", &[p]),
            Issue::Id(p) => said("scene.id", "%1 must match [a-z0-9_]{1,32}", &[p]),
            Issue::DuplicateId(p) => {
                said("scene.duplicate_id", "%1 repeats an id that is already in use", &[p])
            }
            Issue::Unknown { path, what, id } => {
                said("scene.unknown", "%1 names the unknown %2 \"%3\"", &[path, what, id]).texts(&[2])
            }
            Issue::Valueless { path, id } => {
                said("scene.valueless", "%1 names \"%2\", a property that holds no value", &[path, id])
            }
            Issue::Source(p) => said(
                "scene.source",
                "%1 needs exactly one source: prop, keys, audio, cursor, clock or script",
                &[p],
            ),
            Issue::Type(p) => said("scene.type", "%1 does not fit the type of this field", &[p]),
            Issue::Unsorted(p) => said("scene.unsorted", "%1 keyframe times are not increasing", &[p]),
            Issue::Depth(p) => said("scene.depth", "%1 is nested deeper than 8 levels", &[p]),
            Issue::Cycle(p) => said("scene.cycle", "%1 masks itself through other layers", &[p]),
            Issue::Color(c) => {
                said("scene.color", "invalid color \"%1\", expected #rrggbb or #rrggbbaa", &[c])
            }
            Issue::Sound(p) => said(
                "scene.sound",
                "%1 is a sound layer: only id, type, sound, sounds, mode, pause, stream, play_on, loop, play, volume, pan, fade_in and fade_out apply",
                &[p],
            ),
            Issue::Light(p) => said(
                "scene.light",
                "%1 is a light layer: only id, type, transform, anchor, visible, parent, inherit, attach and the fields of the light apply",
                &[p],
            ),
            Issue::Lights(n) => {
                said("scene.lights", "the scene has %1 light layers, at most 12 are allowed", &[n])
            }
            Issue::Variants { path, count } => said(
                "scene.variants",
                "%1 needs %2 variants of its shader, at most 8 are allowed",
                &[path, count],
            ),
            Issue::Clips(p) => said("scene.clips", "%1 needs either sound or sounds", &[p]),
            Issue::Circle(p) => said("scene.circle", "%1 depends on itself through other layers", &[p]),
            Issue::Effect { path, reason } => said("scene.effect", "%1: %2", &[path, reason]).texts(&[2]),
            Issue::SoundSource(p) => {
                said("scene.sound_source", "%1 may only follow a property or keyframes", &[p])
            }
            Issue::Sounds(n) => {
                said("scene.sounds", "the scene has %1 sound layers, at most 8 are allowed", &[n])
            }
            Issue::Volume => said(
                "scene.volume",
                "/properties/volume of a scene with sound must be a slider within 0..=100",
                &[],
            ),
            Issue::Generations(p) => {
                said("scene.generations", "%1 nests particle systems more than two levels deep", &[p])
            }
            Issue::Missing(p, name) => {
                said("scene.missing", "%1 names \"%2\", which its model does not have", &[p, name])
            }
            Issue::Figures(n) => {
                said("scene.figures", "the scene has %1 mesh and model layers, at most 16 are allowed", &[n])
            }
            Issue::Shadows(n) => said(
                "scene.shadows",
                "the scene has %1 lights that cast shadows, at most 2 are allowed",
                &[n],
            ),
            Issue::Flat(p) => said("scene.flat", "%1 applies inside a group with space 3d only", &[p]),
            Issue::Locked(p) => said("scene.locked", "%1 is locked by its particle system", &[p]),
            Issue::Timing(p) => {
                said("scene.timing", "%1 needs either a duration or both frames and fps", &[p])
            }
            Issue::Wrap(p) => said("scene.wrap", "%1 applies to timelines that loop with repeat only", &[p]),
        }
    }
}

impl std::fmt::Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message().fmt(f)
    }
}

impl std::error::Error for Issue {}
