use chrono::{NaiveTime, Weekday};
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::time::Duration;
use wprs_schedule::{Order, format_interval, parse_interval};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rules {
    pub order: Order,
    #[serde(serialize_with = "write_interval", deserialize_with = "read_interval")]
    pub interval: Duration,
    #[serde(serialize_with = "write_times", deserialize_with = "read_times")]
    pub at: Vec<NaiveTime>,
    pub days: Vec<Weekday>,
    pub synced: bool,
    pub resume: bool,
    pub advance_while_paused: bool,
    pub skip_videos_on_battery: bool,
    pub transition_ms: Option<u32>,
}

impl Default for Rules {
    fn default() -> Self {
        Rules {
            order: Order::Sequential,
            interval: Duration::from_secs(1800),
            at: Vec::new(),
            days: Vec::new(),
            synced: false,
            resume: true,
            advance_while_paused: false,
            skip_videos_on_battery: false,
            transition_ms: None,
        }
    }
}

fn write_interval<S: Serializer>(interval: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&format_interval(*interval))
}

fn read_interval<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
    let text = String::deserialize(deserializer)?;
    parse_interval(&text).ok_or_else(|| {
        D::Error::custom(format!("invalid interval {text:?}, expected e.g. \"90s\" or \"1h30m\""))
    })
}

fn write_times<S: Serializer>(times: &[NaiveTime], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(times.iter().map(|time| time.format("%H:%M").to_string()))
}

fn read_times<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<NaiveTime>, D::Error> {
    Vec::<String>::deserialize(deserializer)?
        .iter()
        .map(|text| {
            NaiveTime::parse_from_str(text, "%H:%M")
                .map_err(|_| D::Error::custom(format!("invalid time {text:?}, expected HH:MM")))
        })
        .collect()
}
