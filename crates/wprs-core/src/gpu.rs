use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum GpuChoice {
    #[default]
    Auto,
    Integrated,
    Dedicated,
    Software,
    Named(String),
    Bus(String),
}

impl GpuChoice {
    pub const WORDS: [&str; 4] = ["auto", "integrated", "dedicated", "software"];
    pub const MAX_NAME: usize = 128;
    pub const BUS: &str = "pci:";

    pub fn parse(text: &str) -> Result<GpuChoice, String> {
        let text = text.trim();
        let wrong =
            text.is_empty() || text.chars().count() > Self::MAX_NAME || text.chars().any(char::is_control);
        match text.to_lowercase().as_str() {
            "auto" => Ok(GpuChoice::Auto),
            "integrated" => Ok(GpuChoice::Integrated),
            "dedicated" => Ok(GpuChoice::Dedicated),
            "software" => Ok(GpuChoice::Software),
            lower if lower.starts_with(Self::BUS) => {
                let id = &lower[Self::BUS.len()..];
                let fine = (1..=32).contains(&id.len())
                    && id.chars().all(|c| c.is_ascii_hexdigit() || ":.".contains(c));
                match fine {
                    true => Ok(GpuChoice::Bus(lower.to_owned())),
                    false => Err("pci: takes the PCI bus id of a card, such as pci:0000:03:00.0".into()),
                }
            }
            _ if wrong => Err(format!(
                "expected {} or part of the name of a graphics card, at most {} characters",
                Self::WORDS.join(", "),
                Self::MAX_NAME
            )),
            _ => Ok(GpuChoice::Named(text.into())),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            GpuChoice::Auto => "auto",
            GpuChoice::Integrated => "integrated",
            GpuChoice::Dedicated => "dedicated",
            GpuChoice::Software => "software",
            GpuChoice::Named(name) | GpuChoice::Bus(name) => name,
        }
    }

    pub fn bus(&self) -> Option<&str> {
        match self {
            GpuChoice::Bus(text) => text.strip_prefix(Self::BUS),
            _ => None,
        }
    }

    pub fn is_auto(&self) -> bool {
        *self == GpuChoice::Auto
    }

    pub fn same(&self, other: &GpuChoice) -> bool {
        self.as_str().to_lowercase() == other.as_str().to_lowercase()
    }
}

impl Serialize for GpuChoice {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for GpuChoice {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<GpuChoice, D::Error> {
        GpuChoice::parse(&String::deserialize(d)?).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Render {
    pub gpu: GpuChoice,
}

impl Render {
    pub fn is_default(&self) -> bool {
        self.gpu.is_auto()
    }
}
