use crate::fit::Fit;
use crate::settings::{Pause, Playback, Transition};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub use crate::store::ConfigError;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OutputConfig {
    pub image: Option<PathBuf>,
    pub fit: Option<Fit>,
    pub background: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub default: OutputConfig,
    pub outputs: BTreeMap<String, OutputConfig>,
    pub playback: Playback,
    pub transition: Transition,
    pub pause: Pause,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub image: Option<PathBuf>,
    pub fit: Fit,
    pub background: [u8; 3],
}

impl Config {
    pub fn normalize(&mut self) -> Vec<String> {
        let before = self.options();
        self.playback = self.playback.sanitized();
        self.transition.duration_ms = self.transition.duration_ms.min(Transition::MAX_MS);
        let changed = |(key, value): (String, toml::Value)| (before.get(&key) != Some(&value)).then_some(key);
        let mut fixed: Vec<String> = self.options().into_iter().filter_map(changed).collect();
        let sections = std::iter::once(("default".to_owned(), &mut self.default))
            .chain(self.outputs.iter_mut().map(|(name, o)| (format!("outputs.{name}"), o)));
        for (section, o) in sections {
            if o.background.take_if(|c| parse_hex_color(c).is_none()).is_some() {
                fixed.push(format!("{section}.background"));
            }
        }
        fixed
    }

    pub fn resolve(&self, output: &str) -> Resolved {
        Resolved {
            image: self.pick(output, |o| o.image.clone()),
            fit: self.pick(output, |o| o.fit).unwrap_or_default(),
            background: self
                .pick(output, |o| o.background.as_deref().and_then(parse_hex_color))
                .unwrap_or_default(),
        }
    }

    fn pick<T>(&self, output: &str, field: impl Fn(&OutputConfig) -> Option<T>) -> Option<T> {
        self.outputs.get(output).and_then(&field).or_else(|| field(&self.default))
    }

    pub fn set_image(&mut self, output: &str, image: PathBuf, fit: Option<Fit>) {
        self.section(output, |o| o.image = None).image = Some(image);
        if let Some(fit) = fit {
            self.set_fit(output, fit);
        }
    }

    pub fn set_fit(&mut self, output: &str, fit: Fit) {
        self.section(output, |o| o.fit = None).fit = Some(fit);
    }

    pub fn set_background(&mut self, output: &str, color: &str) -> Result<(), String> {
        let [r, g, b] =
            parse_hex_color(color).ok_or_else(|| format!("invalid color '{color}', expected #rrggbb"))?;
        self.section(output, |o| o.background = None).background = Some(format!("#{r:02x}{g:02x}{b:02x}"));
        Ok(())
    }

    fn section(&mut self, output: &str, clear: impl FnMut(&mut OutputConfig)) -> &mut OutputConfig {
        if !output.is_empty() {
            return self.outputs.entry(output.to_owned()).or_default();
        }
        self.outputs.values_mut().for_each(clear);
        self.outputs.retain(|_, o| *o != OutputConfig::default());
        &mut self.default
    }

    pub fn images(&self) -> impl Iterator<Item = &PathBuf> {
        std::iter::once(&self.default).chain(self.outputs.values()).filter_map(|o| o.image.as_ref())
    }
}

pub fn parse_hex_color(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    (s.len() == 6 && s.is_ascii()).then(|| Some([byte(0)?, byte(2)?, byte(4)?])).flatten()
}
