use crate::pack::{self, Pack};
use crate::{Error, props};
use serde_json::{Value, json};
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use wprs_core::settings::fps::DISPLAY;
use wprs_format::Package;

pub(crate) const LABELS: [&str; 8] = [
    "Speed",
    "Start",
    "Stop at",
    "Volume",
    "Zoom",
    "Horizontal position",
    "Vertical position",
    "0 plays to the end",
];

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Reel {
    pub(crate) rate: f64,
    pub(crate) start: u64,
    pub(crate) end: Option<u64>,
    pub(crate) volume: f64,
    pub(crate) once: bool,
    pub(crate) fps: u32,
    pub(crate) zoom: f32,
    pub(crate) pan: [f32; 2],
}

impl Default for Reel {
    fn default() -> Self {
        Reel {
            rate: 1.0,
            start: 0,
            end: None,
            volume: 1.0,
            once: false,
            fps: DISPLAY,
            zoom: 1.0,
            pan: [0.0; 2],
        }
    }
}

pub(crate) fn labels() -> [String; 8] {
    LABELS.map(String::from)
}

#[cfg(test)]
pub(crate) fn of(package: Option<&std::path::Path>) -> Reel {
    let pack = package.and_then(|path| pack::load(path).inspect_err(|e| log::warn!("{e}")).ok());
    pack.as_deref().map_or_else(Reel::default, played)
}

pub(crate) fn shown(shown: Option<&crate::choice::Shown>) -> Reel {
    let pack = shown.and_then(|s| Some(crate::variant::loaded(s.package.as_deref()?, s.preset.as_deref())));
    pack.and_then(|p| p.inspect_err(|e| log::warn!("{e}")).ok()).as_deref().map_or_else(Reel::default, played)
}

pub(crate) fn played(pack: &Pack) -> Reel {
    let Some(manifest) = pack.video.as_ref() else { return Reel::default() };
    let values = props::values(pack);
    let num = |id: &str| values.get(id).and_then(Value::as_f64).unwrap_or(0.0);
    let (start, end) = ((num("start") * 1000.0) as u64, (num("end") * 1000.0) as u64);
    let playback = manifest.playback.clone().unwrap_or_default();
    Reel {
        rate: num("speed").clamp(0.1, 4.0),
        start,
        end: (end > start + 100).then_some(end),
        volume: (num("volume") / 100.0).clamp(0.0, 1.0),
        once: playback.once(),
        fps: playback.frame_cap(),
        zoom: (num("zoom") as f32).clamp(1.0, 8.0),
        pan: [num("x") as f32, num("y") as f32].map(|p| p.clamp(-1.0, 1.0)),
    }
}

impl Reel {
    pub(crate) fn framed(&self) -> bool {
        self.zoom != 1.0 || self.pan != [0.0; 2]
    }

    pub(crate) fn direct(&self) -> bool {
        !self.framed()
    }

    pub(crate) fn cap(&self) -> u32 {
        if self.fps == DISPLAY { 0 } else { self.fps }
    }

    pub(crate) fn told(&self) -> Value {
        let end = self.end.map_or(-1, |end| end as i64);
        json!({ "rate": self.rate, "start": self.start, "end": end, "loop": !self.once, "volume": self.volume,
            "fps": self.cap() })
    }
}

pub(crate) fn pack(package: &Package<BufReader<File>>) -> Result<Pack, Error> {
    let m = package.manifest.localized();
    let properties = m.video_properties(&labels(), None);
    let labels = wprs_scene::labels(&properties, &m.default_locale.iter().cloned().collect::<Vec<_>>());
    Ok(Pack {
        id: m.id.clone(),
        version: m.version.clone(),
        locale: m.default_locale.clone(),
        scene: Arc::new(pack::blank()?),
        properties,
        presets: m.presets.clone(),
        assets: Default::default(),
        videos: Vec::new(),
        sounds: Vec::new(),
        inputs: None,
        scripts: Vec::new(),
        labels,
        web: None,
        video: Some(Arc::new(m)),
        tried: None,
    })
}
