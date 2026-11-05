use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

mod glows;
mod lamps;
mod limbs;
mod marks;
mod motion;
mod names;
mod rigging;
mod shaping;
mod solid;
mod sow;
mod swarm;
mod tend;
pub use lamps::{AmbientEdit, LightEdit, LightSet, LightingEdit, Map, MapStroke, NormalsMake};
pub use limbs::{AttachEdit, BodyEdit, IkEdit, PartsEdit, PointEdit, Spot, WindEdit};
pub use marks::{CanvasEdit, LayerEdit, ManifestEdit, PropertyEdit};
pub use motion::{AnimEdit, PathEdit, PathKey, PlayEdit, ShadowEdit};
pub use names::{ACTIONS, AssetEdit, ConditionEdit, EDITOR, ImportAssets, RgbEdit, ScriptFile, UpgradeEdit};
pub use rigging::{
    AutoWeights, BoneEdit, ClipEdit, ClipKey, DepthStroke, ExpressionEdit, KeyEase, MaskStroke, MeshCut,
    WeightStroke,
};
pub use shaping::EffectsEdit;
pub use solid::{CameraEdit, Edit3d, MaterialEdit, ModelEdit, ModelSet, TonesEdit, ViewEdit};
pub use sow::SowEdit;
pub use swarm::{ParticleEdit, Response, Strand, SwarmLight, TendEdit};

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
        #[serde(default)]
        linear: bool,
        #[serde(default)]
        atlas: Option<[u32; 2]>,
    },
    ImportEffect {
        descriptor: PathBuf,
        #[serde(default)]
        wgsl: Option<PathBuf>,
    },
    ImportScript(ScriptFile),
    ImportAssets(ImportAssets),
    Asset(AssetEdit),
    Rgb(RgbEdit),
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
    Upgrade(UpgradeEdit),
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
    Condition(ConditionEdit),
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
    Scene3d(Edit3d),
    Animate(AnimEdit),
    Effects(EffectsEdit),
    Layer(LayerEdit),
    Manifest(ManifestEdit),
    Property(PropertyEdit),
    Canvas(CanvasEdit),
    Undo {},
    Redo {},
}
