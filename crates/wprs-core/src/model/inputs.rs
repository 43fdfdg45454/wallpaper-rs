use std::collections::{BTreeMap, BTreeSet};

model! {
    #[derive(Copy, Eq, Hash, PartialOrd, Ord)]
    pub enum Input { Audio, Pointer }

    pub struct SceneInputs {
        pub scene: String,
        pub audio: bool,
        pub pointer: bool,
        #[serde(default)]
        pub title: String,
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
    }
}

impl Input {
    pub fn parse(name: &str) -> Option<Input> {
        [Input::Audio, Input::Pointer].into_iter().find(|i| i.as_str() == name)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Input::Audio => "audio",
            Input::Pointer => "pointer",
        }
    }
}
