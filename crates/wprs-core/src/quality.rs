use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum QualityChoice {
    #[default]
    Auto,
    Fixed(u8),
    AtLeast(u8),
}

impl QualityChoice {
    pub const WORDS: [&str; 7] = ["auto", "q0", "q1", "q2", "q3", "min-q1", "min-q2"];
    pub const TOP: u8 = 3;
    pub const KEY: &str = "playback.quality";
    pub const OWN: &str = "playback.quality.";

    pub fn parse(text: &str) -> Result<QualityChoice, String> {
        let text = text.trim().to_lowercase();
        let level = |t: &str| {
            t.strip_prefix('q').filter(|d| d.len() == 1)?.parse::<u8>().ok().filter(|l| *l <= Self::TOP)
        };
        let floor = |t: &str| level(t.strip_prefix("min-")?).filter(|l| (1..Self::TOP).contains(l));
        let fixed = level(&text).map(QualityChoice::Fixed);
        let found = fixed.or_else(|| floor(&text).map(QualityChoice::AtLeast));
        match text.as_str() {
            "auto" => Ok(QualityChoice::Auto),
            _ => found.ok_or_else(|| format!("expected {}", Self::WORDS.join(", "))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            QualityChoice::Auto => "auto",
            QualityChoice::Fixed(l) => ["q0", "q1", "q2", "q3"][usize::from(l.min(Self::TOP))],
            QualityChoice::AtLeast(l) => ["auto", "min-q1", "min-q2", "q3"][usize::from(l.min(Self::TOP))],
        }
    }

    pub fn is_auto(&self) -> bool {
        *self == QualityChoice::Auto
    }

    pub fn floor(self) -> u8 {
        match self {
            QualityChoice::Auto => 0,
            QualityChoice::Fixed(l) | QualityChoice::AtLeast(l) => l.min(Self::TOP),
        }
    }

    pub fn fixed(self) -> Option<u8> {
        match self {
            QualityChoice::Fixed(l) => Some(l.min(Self::TOP)),
            _ => None,
        }
    }
}

impl Serialize for QualityChoice {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for QualityChoice {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<QualityChoice, D::Error> {
        QualityChoice::parse(&String::deserialize(d)?).map_err(D::Error::custom)
    }
}
