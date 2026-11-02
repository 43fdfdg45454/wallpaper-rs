use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use wprs_format::{Descriptor, Rating};

mod glows;
mod lamps;
mod limbs;
mod names;
mod rigging;
mod swarm;
mod tend;

pub use glows::SwarmLight;
pub use lamps::{AmbientEdit, LightEdit, LightSet, LightingEdit, Map, MapStroke, NormalsMake};
pub use limbs::{AttachEdit, BodyEdit, IkEdit, PartsEdit, PointEdit, Spot, WindEdit};
pub use names::ACTIONS;
pub use rigging::{
    AutoWeights, BoneEdit, ClipEdit, ClipKey, DepthStroke, ExpressionEdit, KeyEase, MaskStroke, MeshCut,
    WeightStroke,
};
pub use swarm::{ParticleEdit, Response, Strand, TendEdit};

pub const EDITOR: &str = "/scene/ext/editor";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Set {
        pointer: String,
        value: Value,
    },
    Add {
        pointer: String,
        value: Value,
    },
    Remove {
        pointer: String,
    },
    Move {
        from: String,
        to: String,
    },
    AddLayer {
        kind: String,
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        source: Option<String>,
        #[serde(default)]
        into: Option<String>,
        #[serde(default)]
        at: Option<usize>,
    },
    RemoveLayer {
        id: String,
    },
    DuplicateLayer {
        id: String,
    },
    MoveLayer {
        id: String,
        to: usize,
        #[serde(default)]
        into: Option<String>,
    },
    Group {
        ids: Vec<String>,
        #[serde(default)]
        id: Option<String>,
    },
    Ungroup {
        id: String,
    },
    Hide {
        id: String,
        hidden: bool,
    },
    Lock {
        id: String,
        locked: bool,
    },
    Solo {
        #[serde(default)]
        id: Option<String>,
    },
    Keyframe {
        pointer: String,
        time: f64,
        #[serde(default)]
        value: Option<Value>,
        #[serde(default)]
        ease: Option<Value>,
        #[serde(default)]
        timeline: Option<String>,
        #[serde(default)]
        remove: bool,
        #[serde(default)]
        from: Option<f64>,
    },
    Import {
        file: PathBuf,
        #[serde(default)]
        id: Option<String>,
    },
    ImportEffect {
        descriptor: PathBuf,
        #[serde(default)]
        wgsl: Option<PathBuf>,
    },
    ImportScript {
        file: PathBuf,
    },
    Translate {
        pointer: String,
        language: String,
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        add: bool,
    },
    Thumbnail {
        file: PathBuf,
        #[serde(default)]
        animated: bool,
    },
    Upgrade {
        #[serde(default)]
        rating: Option<Rating>,
        #[serde(default)]
        reasons: Vec<Descriptor>,
        #[serde(default)]
        photosensitive: Option<bool>,
        #[serde(default)]
        derived_from: Vec<String>,
    },
    AddProperty {
        kind: String,
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        group: Option<String>,
        #[serde(default)]
        label: Option<String>,
    },
    MoveProperty {
        id: String,
        to: usize,
        #[serde(default)]
        group: Option<String>,
    },
    RenameProperty {
        id: String,
        to: String,
    },
    RemoveProperty {
        id: String,
    },
    Condition {
        id: String,
        #[serde(default)]
        enabled: bool,
        #[serde(default)]
        text: String,
    },
    AddPreset {
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        label: Option<String>,
    },
    PaintMask(MaskStroke),
    MeshFromPicture(MeshCut),
    Bone(BoneEdit),
    MeshDepth(DepthStroke),
    Weights(WeightStroke),
    AutoWeights(AutoWeights),
    Clip(ClipEdit),
    Key(ClipKey),
    Expression(ExpressionEdit),
    Ik(IkEdit),
    Physics(BodyEdit),
    Wind(WindEdit),
    Attach(AttachEdit),
    Point(PointEdit),
    Parts(PartsEdit),
    Particles(ParticleEdit),
    Light(LightEdit),
    Lighting(LightingEdit),
    Normals(NormalsMake),
    PaintMap(MapStroke),
    Ambient(AmbientEdit),
    Undo {},
    Redo {},
}
