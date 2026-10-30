use crate::Fit;
use serde_json::Value;

model! {
    pub struct Schema {
        pub model: u32,
        pub serial: u64,
        pub options: Vec<Field>,
        pub rules: Vec<Field>,
        pub auto: Vec<Field>,
        pub preset: Vec<Field>,
        pub providers: Vec<ProviderKind>,
        pub query: Vec<QueryKey>,
        pub thumb_sizes: Vec<u32>,
        pub fits: Vec<Fit>,
    }

    pub struct Field {
        pub key: String,
        pub kind: FieldKind,
        pub default: Value,
        pub min: Option<f64>,
        pub max: Option<f64>,
        pub step: Option<f64>,
        pub values: Vec<String>,
        pub required: bool,
        pub clearable: bool,
    }

    #[derive(Copy, Eq, Hash)]
    pub enum FieldKind {
        Bool, Int, Float, Text, Enum, Color, Interval, Times, Weekdays, List, Url,
        #[serde(other)] Unknown,
    }

    pub struct ProviderKind {
        pub kind: String,
        pub hosts: Vec<String>,
        pub options: Vec<Field>,
        pub secret: Option<SecretField>,
    }

    pub struct SecretField { pub name: String, pub required: bool }

    pub struct QueryKey { pub key: String, pub hint: String }
}
