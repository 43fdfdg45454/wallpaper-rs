use serde::Deserialize;
use serde::de::IgnoredAny;
use std::collections::BTreeMap;

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Gltf {
    pub nodes: Vec<Node>,
    pub skins: Vec<Skin>,
    pub meshes: Vec<Mesh>,
    pub accessors: Vec<Accessor>,
    #[serde(rename = "bufferViews")]
    pub views: Vec<View>,
    pub animations: Vec<Animation>,
    pub materials: Vec<Material>,
    pub textures: Vec<Texture>,
    pub images: Vec<Image>,
    #[serde(rename = "extensionsRequired")]
    pub required: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Node {
    pub name: Option<String>,
    pub children: Vec<usize>,
    pub mesh: Option<usize>,
    pub skin: Option<usize>,
    pub translation: Option<[f32; 3]>,
    pub rotation: Option<[f32; 4]>,
    pub scale: Option<[f32; 3]>,
    pub matrix: Option<[f32; 16]>,
    pub weights: Vec<f32>,
    pub extras: serde_json::Value,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Skin {
    pub joints: Vec<usize>,
    #[serde(rename = "inverseBindMatrices")]
    pub inverse: Option<usize>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Mesh {
    pub name: Option<String>,
    pub primitives: Vec<Primitive>,
    pub weights: Vec<f32>,
    pub extras: Option<Extras>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Extras {
    #[serde(rename = "targetNames")]
    pub names: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Primitive {
    pub attributes: BTreeMap<String, usize>,
    pub indices: Option<usize>,
    pub mode: Option<u32>,
    pub material: Option<usize>,
    pub targets: Vec<BTreeMap<String, usize>>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Accessor {
    #[serde(rename = "bufferView")]
    pub view: Option<usize>,
    #[serde(rename = "byteOffset")]
    pub offset: usize,
    #[serde(rename = "componentType")]
    pub component: u32,
    pub normalized: bool,
    pub count: usize,
    #[serde(rename = "type")]
    pub kind: String,
    pub sparse: Option<IgnoredAny>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct View {
    pub buffer: usize,
    #[serde(rename = "byteOffset")]
    pub offset: usize,
    #[serde(rename = "byteLength")]
    pub length: usize,
    #[serde(rename = "byteStride")]
    pub stride: Option<usize>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Animation {
    pub name: Option<String>,
    pub channels: Vec<Channel>,
    pub samplers: Vec<Sampler>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Channel {
    pub sampler: usize,
    pub target: Target,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Target {
    pub node: Option<usize>,
    pub path: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Sampler {
    pub input: usize,
    pub output: usize,
    pub interpolation: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Material {
    pub name: Option<String>,
    #[serde(rename = "pbrMetallicRoughness")]
    pub pbr: Option<Pbr>,
    #[serde(rename = "normalTexture")]
    pub normal: Option<Reference>,
    #[serde(rename = "occlusionTexture")]
    pub occlusion: Option<Reference>,
    #[serde(rename = "emissiveTexture")]
    pub glow: Option<Reference>,
    #[serde(rename = "emissiveFactor")]
    pub emissive: Option<[f32; 3]>,
    #[serde(rename = "alphaMode")]
    pub alpha: Option<String>,
    #[serde(rename = "alphaCutoff")]
    pub cutoff: Option<f32>,
    #[serde(rename = "doubleSided")]
    pub double: bool,
    pub extensions: BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Pbr {
    #[serde(rename = "baseColorFactor")]
    pub color: Option<[f32; 4]>,
    #[serde(rename = "baseColorTexture")]
    pub texture: Option<Reference>,
    #[serde(rename = "metallicFactor")]
    pub metallic: Option<f32>,
    #[serde(rename = "roughnessFactor")]
    pub roughness: Option<f32>,
    #[serde(rename = "metallicRoughnessTexture")]
    pub orm: Option<Reference>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Reference {
    pub index: usize,
    #[serde(rename = "texCoord")]
    pub set: usize,
    pub scale: Option<f32>,
    pub strength: Option<f32>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Texture {
    pub source: Option<usize>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(crate) struct Image {
    #[serde(rename = "bufferView")]
    pub view: Option<usize>,
    #[serde(rename = "mimeType")]
    pub mime: Option<String>,
}
