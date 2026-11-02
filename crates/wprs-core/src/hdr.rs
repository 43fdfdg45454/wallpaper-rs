use crate::config::{Config, OutputConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HdrChoice {
    #[default]
    Auto,
    On,
    Off,
}

pub const UNKNOWN_HEADROOM: f32 = 2.0;
pub const MAX_HEADROOM: f32 = 64.0;

impl HdrChoice {
    pub const WORDS: [&str; 3] = ["auto", "on", "off"];
    pub const KEY: &str = "playback.hdr";
    pub const OWN: &str = "playback.hdr.";

    pub fn parse(text: &str) -> Result<HdrChoice, String> {
        match text.trim().to_lowercase().as_str() {
            "auto" => Ok(HdrChoice::Auto),
            "on" => Ok(HdrChoice::On),
            "off" => Ok(HdrChoice::Off),
            _ => Err(format!("expected {}", Self::WORDS.join(", "))),
        }
    }

    pub fn as_str(self) -> &'static str {
        Self::WORDS[self as usize]
    }

    pub fn decide(
        self,
        (offered, display): (bool, bool),
        on_battery: bool,
        top: bool,
    ) -> (bool, &'static str) {
        match self {
            HdrChoice::Off => (false, "off"),
            _ if !offered => (false, "display"),
            HdrChoice::On => (true, "on"),
            HdrChoice::Auto if !display => (false, "display"),
            HdrChoice::Auto if on_battery => (false, "battery"),
            HdrChoice::Auto if !top => (false, "quality"),
            HdrChoice::Auto => (true, "auto"),
        }
    }
}

pub fn headroom(peak_nits: Option<f32>, reference_nits: Option<f32>) -> f32 {
    match (peak_nits, reference_nits) {
        (Some(p), Some(r)) if p.is_finite() && r.is_finite() && r > 0.0 && p > 0.0 => {
            (p / r).clamp(1.0, MAX_HEADROOM)
        }
        _ => UNKNOWN_HEADROOM,
    }
}

impl Config {
    pub fn hdr(&self, output: &str) -> HdrChoice {
        self.own_hdr(output).unwrap_or(self.playback.hdr)
    }

    pub fn own_hdr(&self, output: &str) -> Option<HdrChoice> {
        self.outputs.get(output).filter(|_| !output.is_empty()).and_then(|o| o.hdr)
    }

    pub fn own_hdrs(&self) -> std::collections::BTreeMap<String, String> {
        self.outputs.iter().filter_map(|(name, o)| Some((name.clone(), o.hdr?.as_str().into()))).collect()
    }

    pub(crate) fn set_own_hdr(&mut self, output: &str, value: &str) -> Result<(), String> {
        let (output, text) = (output.trim(), value.trim());
        if output.is_empty() || output.len() > 128 || output.chars().any(char::is_control) {
            return Err(format!("invalid output name in {}<output>", HdrChoice::OWN));
        }
        let own = match text {
            "" | "none" => None,
            _ => Some(
                HdrChoice::parse(text)
                    .map_err(|e| format!("invalid value for {}{output}: {e}", HdrChoice::OWN))?,
            ),
        };
        match self.outputs.get_mut(output) {
            Some(o) => o.hdr = own,
            None if own.is_some() => self.outputs.entry(output.to_owned()).or_default().hdr = own,
            None => {}
        }
        self.outputs.retain(|_, o| *o != OutputConfig::default());
        Ok(())
    }
}
