use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

const KNOWN: [(&str, &str); 1] = [("KineticWE", "noctalia-wallpaper")];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerNamespace(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Option,
    Environment,
    Config,
    Automatic,
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen {
    pub name: String,
    pub from: Source,
    pub ignored: Option<String>,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Option => "option",
            Source::Environment => "environment",
            Source::Config => "config",
            Source::Automatic => "automatic",
            Source::Default => "default",
        }
    }
}

impl LayerNamespace {
    pub const KEY: &str = "surfaces.layer_namespace";
    pub const ENV: &str = "WPRS_LAYER_NAMESPACE";
    pub const DEFAULT: &str = "wallpaper";
    pub const AUTO: &str = "auto";
    pub const MAX: usize = 64;

    pub fn parse(text: &str) -> Result<Option<LayerNamespace>, String> {
        let text = text.trim();
        if text.is_empty() || text.eq_ignore_ascii_case(Self::AUTO) {
            return Ok(None);
        }
        let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
        if text.len() > Self::MAX || !text.chars().all(allowed) {
            return Err(format!(
                "expected {} or a name of 1 to {} letters, digits, '-', '_' or '.'",
                Self::AUTO,
                Self::MAX
            ));
        }
        Ok(Some(LayerNamespace(text.into())))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn automatic(desktop: &str) -> Option<&'static str> {
        let known = |d: &str| KNOWN.iter().find(|(name, _)| d.trim().eq_ignore_ascii_case(name));
        desktop.split(':').find_map(known).map(|(_, namespace)| *namespace)
    }

    pub fn choose(
        option: Option<&str>,
        environment: Option<&str>,
        config: Option<&LayerNamespace>,
        desktop: &str,
    ) -> Chosen {
        let mut ignored = None;
        let mut given = |text: Option<&str>, from: Source| match LayerNamespace::parse(text?) {
            Ok(found) => found.map(|n| (n.0, from)),
            Err(e) => {
                ignored = Some(format!("the {} value is ignored: {e}", from.as_str()));
                None
            }
        };
        let (name, from) = given(option, Source::Option)
            .or_else(|| given(environment, Source::Environment))
            .or_else(|| config.map(|n| (n.0.clone(), Source::Config)))
            .or_else(|| Self::automatic(desktop).map(|n| (n.to_owned(), Source::Automatic)))
            .unwrap_or_else(|| (Self::DEFAULT.to_owned(), Source::Default));
        Chosen { name, from, ignored }
    }
}

impl Serialize for LayerNamespace {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Surfaces {
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "read")]
    pub layer_namespace: Option<LayerNamespace>,
}

impl Surfaces {
    pub fn is_default(&self) -> bool {
        self.layer_namespace.is_none()
    }

    pub fn layer_namespace(&self) -> &str {
        self.layer_namespace.as_ref().map_or(LayerNamespace::AUTO, LayerNamespace::as_str)
    }
}

fn read<'de, D: Deserializer<'de>>(d: D) -> Result<Option<LayerNamespace>, D::Error> {
    LayerNamespace::parse(&String::deserialize(d)?).map_err(D::Error::custom)
}
