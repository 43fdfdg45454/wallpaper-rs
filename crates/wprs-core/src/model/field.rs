use serde_json::Value;

model! {
    pub struct SceneField {
        pub key: String,
        pub kind: String,
        pub label: String,
        pub help: String,
        pub value: Value,
        pub visible: bool,
        pub min: Option<f64>,
        pub max: Option<f64>,
        pub step: Option<f64>,
        pub integer: bool,
        pub unit: String,
        pub options: Vec<SceneOption>,
        pub alpha: bool,
        pub multiline: bool,
        pub max_len: Option<u32>,
        pub accept: Vec<String>,
        #[serde(default = "yes")]
        pub enabled: bool,
        #[serde(default)]
        pub group: String,
        #[serde(default)]
        pub icon: String,
        #[serde(default)]
        pub decimals: Option<u32>,
        #[serde(default)]
        pub editable: bool,
        #[serde(default)]
        pub log: bool,
        #[serde(default)]
        pub collapsed: bool,
        #[serde(default)]
        pub secret: bool,
        #[serde(default)]
        pub filled: bool,
    }

    pub struct SceneOption {
        pub value: Value,
        pub label: String,
        #[serde(default)]
        pub icon: String,
    }
}

fn yes() -> bool {
    true
}
