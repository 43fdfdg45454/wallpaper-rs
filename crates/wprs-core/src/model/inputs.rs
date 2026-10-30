use std::collections::{BTreeMap, BTreeSet};

model! {
    #[derive(Copy, Eq, Hash, PartialOrd, Ord)]
    pub enum Input { Audio, Pointer, Clicks }

    pub struct SceneInputs {
        pub scene: String,
        pub audio: bool,
        pub pointer: bool,
        #[serde(default)]
        pub clicks: bool,
        #[serde(default)]
        pub title: String,
        #[serde(default)]
        pub key: String,
        #[serde(default)]
        pub granted: BTreeSet<Input>,
    }

    pub struct InputsReply {
        pub model: u32,
        pub serial: u64,
        pub capturing: bool,
        pub tracking: bool,
        pub grants: BTreeMap<String, BTreeSet<Input>>,
        pub shown: BTreeMap<String, SceneInputs>,
        #[serde(default)]
        pub parallax: BTreeSet<String>,
        #[serde(default)]
        pub declined: BTreeMap<String, BTreeSet<Input>>,
        #[serde(default)]
        pub asking: BTreeMap<String, BTreeSet<Input>>,
        #[serde(default)]
        pub quiet: BTreeMap<String, BTreeSet<Input>>,
        #[serde(default)]
        pub clicks: BTreeSet<String>,
        #[serde(default)]
        pub clicking: BTreeSet<String>,
    }
}

impl Input {
    pub const ALL: [Input; 3] = [Input::Audio, Input::Pointer, Input::Clicks];

    pub fn parse(name: &str) -> Option<Input> {
        Input::ALL.into_iter().find(|i| i.as_str() == name)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Input::Audio => "audio",
            Input::Pointer => "pointer",
            Input::Clicks => "clicks",
        }
    }
}
