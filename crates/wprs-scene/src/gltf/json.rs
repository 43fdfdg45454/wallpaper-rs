use serde::Deserialize;
use serde::de::{self, DeserializeOwned, Deserializer, IgnoredAny, SeqAccess, Visitor};
use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;

thread_local! {
    static LEFT: Cell<usize> = const { Cell::new(0) };
}

pub(super) fn within<T>(left: &mut usize, run: impl FnOnce() -> T) -> T {
    LEFT.set(*left);
    let out = run();
    *left = LEFT.get();
    out
}

fn listed<'de, D: Deserializer<'de>, T: DeserializeOwned>(d: D) -> Result<Vec<T>, D::Error> {
    struct List<T>(PhantomData<T>);
    impl<'de, T: DeserializeOwned> Visitor<'de> for List<T> {
        type Value = Vec<T>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a list")
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<T>, A::Error> {
            let mut out = Vec::new();
            loop {
                let left = LEFT.get().checked_sub(1).ok_or_else(|| de::Error::custom(super::CROWDED))?;
                LEFT.set(left);
                match seq.next_element()? {
                    Some(item) => out.push(item),
                    None => return Ok(out),
                }
            }
        }
    }
    d.deserialize_seq(List(PhantomData))
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Doc {
    pub asset: Info,
    #[serde(deserialize_with = "listed")]
    pub nodes: Vec<NodeHead>,
    #[serde(deserialize_with = "listed")]
    pub skins: Vec<Skin>,
    #[serde(deserialize_with = "listed")]
    pub meshes: Vec<Mesh>,
    #[serde(deserialize_with = "listed")]
    pub animations: Vec<Animation>,
    #[serde(deserialize_with = "listed")]
    pub accessors: Vec<Accessor>,
    #[serde(deserialize_with = "listed")]
    pub materials: Vec<Named>,
    #[serde(deserialize_with = "listed")]
    pub buffers: Vec<Linked>,
    #[serde(deserialize_with = "listed")]
    pub images: Vec<Linked>,
    #[serde(rename = "extensionsRequired", deserialize_with = "listed")]
    pub required: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Info {
    pub version: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Named {
    pub name: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct NodeHead {
    pub name: Option<String>,
    #[serde(deserialize_with = "listed")]
    pub children: Vec<usize>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Linked {
    pub uri: Option<IgnoredAny>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Skin {
    #[serde(deserialize_with = "listed")]
    pub joints: Vec<usize>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Mesh {
    pub name: Option<String>,
    #[serde(deserialize_with = "listed")]
    pub primitives: Vec<Primitive>,
    pub extras: Option<Extras>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Extras {
    #[serde(rename = "targetNames", deserialize_with = "listed")]
    pub names: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Primitive {
    pub attributes: Attributes,
    pub indices: Option<usize>,
    pub mode: Option<u32>,
    #[serde(deserialize_with = "listed")]
    pub targets: Vec<IgnoredAny>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Attributes {
    #[serde(rename = "POSITION")]
    pub position: Option<usize>,
    #[serde(rename = "JOINTS_0")]
    pub joints: Option<IgnoredAny>,
    #[serde(rename = "WEIGHTS_0")]
    pub weights: Option<IgnoredAny>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Animation {
    pub name: Option<String>,
    #[serde(deserialize_with = "listed")]
    pub samplers: Vec<Sampler>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Sampler {
    pub input: usize,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub(super) struct Accessor {
    pub count: u64,
    #[serde(deserialize_with = "listed")]
    pub max: Vec<f64>,
}
