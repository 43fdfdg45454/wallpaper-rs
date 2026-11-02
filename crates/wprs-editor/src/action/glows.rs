use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SwarmLight {
    Lit {
        system: String,
        #[serde(default)]
        layer: Option<String>,
        #[serde(default)]
        lit: Option<bool>,
        #[serde(default)]
        normal: Option<String>,
    },
    Refract {
        system: String,
        #[serde(default)]
        layer: Option<String>,
        #[serde(default)]
        normal: Option<String>,
        #[serde(default)]
        amount: Option<f64>,
        #[serde(default)]
        off: bool,
    },
}
