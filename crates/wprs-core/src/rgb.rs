use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod places;

pub use places::{Flow, MAX_PLACES, PLACE, Place};

pub const SERVER: &str = "127.0.0.1:6742";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Covered {
    #[default]
    Pause,
    Follow,
}

impl Covered {
    pub const KEY: &str = "rgb.when_covered";
    pub const WORDS: [&str; 2] = ["pause", "follow"];

    pub fn as_str(self) -> &'static str {
        Self::WORDS[self as usize]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rgb {
    pub enabled: bool,
    pub server: String,
    pub when_covered: Covered,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub places: BTreeMap<String, String>,
}

impl Default for Rgb {
    fn default() -> Self {
        Self { enabled: false, server: SERVER.into(), when_covered: Covered::Pause, places: BTreeMap::new() }
    }
}

fn split(server: &str) -> Option<(&str, u16)> {
    let (host, port) = server.rsplit_once(':')?;
    let port = port.parse::<u16>().ok().filter(|p| *p > 0)?;
    let host = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host);
    let fine = |c: char| c.is_ascii_alphanumeric() || ".-:_".contains(c);
    (!host.is_empty() && host.chars().all(fine)).then_some((host, port))
}

pub fn valid(server: &str) -> bool {
    server.len() <= 255 && split(server).is_some()
}

impl Rgb {
    pub fn sanitized(mut self) -> Rgb {
        self.tidy_places();
        match valid(&self.server) {
            true => self,
            false => Rgb { server: SERVER.into(), ..self },
        }
    }

    pub fn local(&self) -> bool {
        split(&self.server).is_some_and(|(host, _)| {
            host == "localhost"
                || host == "::1"
                || host.parse::<std::net::Ipv4Addr>().is_ok_and(|ip| ip.is_loopback())
        })
    }
}
