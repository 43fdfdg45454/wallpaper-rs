use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    #[default]
    Sequential,
    Shuffle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Playlist {
    pub id: String,
    pub items: Vec<PathBuf>,
    #[serde(default)]
    pub order: Order,
    #[serde(with = "interval")]
    pub interval: Duration,
    #[serde(default)]
    pub per_output: bool,
}

pub fn parse_interval(text: &str) -> Option<Duration> {
    let (mut total, mut number) = (0_u64, None::<u64>);
    for c in text.chars().filter(|c| !c.is_whitespace()) {
        match c.to_digit(10) {
            Some(d) => number = Some(number.unwrap_or(0).checked_mul(10)?.checked_add(d.into())?),
            None => {
                let (_, factor) = UNITS.iter().find(|(unit, _)| *unit == c)?;
                total = total.checked_add(number.take()?.checked_mul(*factor)?)?;
            }
        }
    }
    (number.is_none() && !text.trim().is_empty()).then_some(Duration::from_secs(total))
}

pub fn format_interval(duration: Duration) -> String {
    let mut rest = duration.as_secs();
    let text: String = UNITS
        .iter()
        .filter_map(|(unit, factor)| {
            let n = rest / factor;
            rest %= factor;
            (n > 0).then(|| format!("{n}{unit}"))
        })
        .collect();
    if text.is_empty() { "0s".into() } else { text }
}

const UNITS: [(char, u64); 4] = [('d', 86_400), ('h', 3_600), ('m', 60), ('s', 1)];

mod interval {
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&super::format_interval(*duration))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
        let text = String::deserialize(deserializer)?;
        super::parse_interval(&text).ok_or_else(|| {
            D::Error::custom(format!("invalid interval {text:?}, expected e.g. \"90s\" or \"1h30m\""))
        })
    }
}
