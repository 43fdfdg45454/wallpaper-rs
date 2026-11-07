use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Fill {
    Lama,
    Simple,
    Off,
}

impl Fill {
    pub const WORDS: [&str; 3] = ["lama", "simple", "off"];
    pub const KEY: &str = "parallax.fill";

    pub fn parse(text: &str) -> Result<Fill, String> {
        match text.trim().to_lowercase().as_str() {
            "lama" => Ok(Fill::Lama),
            "simple" => Ok(Fill::Simple),
            "off" => Ok(Fill::Off),
            _ => Err(format!("expected {}", Self::WORDS.join(", "))),
        }
    }

    pub fn as_str(self) -> &'static str {
        Self::WORDS[self as usize]
    }

    pub fn standard() -> Fill {
        if cfg!(target_os = "android") { Fill::Simple } else { Fill::Lama }
    }
}

impl Serialize for Fill {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Fill {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Fill, D::Error> {
        Fill::parse(&String::deserialize(d)?).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Parallax {
    pub fill: Fill,
    pub layers: u8,
}

impl Parallax {
    pub const LAYERS: std::ops::RangeInclusive<u8> = 4..=6;

    pub fn sanitized(&self) -> Parallax {
        Parallax { layers: self.layers.clamp(*Self::LAYERS.start(), *Self::LAYERS.end()), ..self.clone() }
    }
}

impl Default for Parallax {
    fn default() -> Self {
        Self { fill: Fill::standard(), layers: 5 }
    }
}
