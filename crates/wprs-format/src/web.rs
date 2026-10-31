use crate::names::{valid_id, valid_property};
use crate::needs::Dependency;
use crate::{Accent, Error, Input, Kind, Manifest, Result, Span, check_path, require};
use schemars::JsonSchema;
use semver::VersionReq;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use wprs_scene::{Control, Values};

pub const MAX_STORAGE_KIB: u32 = 5120;
pub const MAX_PRESET_VALUES: usize = 256;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Web {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preload: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-color" = true))]
    pub background: Option<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    #[schemars(extend("x-unit" = "KiB", "x-range" = [0, 5120]))]
    pub storage_kib: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PresetFile {
    pub base: Dependency,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: Values,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub files: BTreeMap<String, String>,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

pub(crate) fn rgb(text: &str) -> bool {
    text.strip_prefix('#').is_some_and(|h| h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()))
}

impl Manifest {
    pub(crate) fn check_kind(&self) -> Result<()> {
        let web = self.kind == Kind::Web;
        let allow = self.permissions.iter().flat_map(|p| &p.network).flat_map(|n| &n.allow);
        let networked = allow.clone().next().is_some();
        let block = self.web.clone().unwrap_or_default();
        let content = !matches!(self.kind, Kind::Preset | Kind::Assets);
        let based = self.kind != Kind::Preset || !self.dependencies().is_empty();
        let packages = self.requires.as_ref().is_none_or(|r| r.valid_packages(&self.id));
        let rules = [
            ("web", web || self.web.is_none(), "applies to web packages only"),
            (
                "permissions",
                matches!(self.kind, Kind::Web | Kind::Scene) || self.permissions.is_none(),
                "applies to web and scene packages only",
            ),
            ("permissions.network", web || !networked, "applies to web packages only"),
            ("web.storage_kib", block.storage_kib <= MAX_STORAGE_KIB, "expected 0 to 5120 KiB"),
            ("web.background", block.background.as_deref().is_none_or(rgb), "expected a #rrggbb color"),
            (
                "permissions.network.allow",
                self.permissions.iter().flat_map(|p| &p.network).all(|n| n.valid()),
                "expected at most 8 different https:// origins such as https://api.example.org",
            ),
            (
                "inputs",
                networked == self.inputs.contains(&Input::Network) || !web,
                "a web package that lists origins in permissions.network.allow declares network, and only then",
            ),
            (
                "requires.packages",
                packages,
                "at most 16 other package ids, each once, with a version requirement such as ^1.2",
            ),
            ("requires.packages", based, "lists the base of the preset"),
            ("properties", content || self.properties.is_empty(), "preset and assets packages have none"),
            ("presets", content || self.presets.is_empty(), "preset and assets packages have none"),
            ("inputs", content || self.inputs.is_empty(), "preset and assets packages use none"),
        ];
        rules.iter().try_for_each(|(field, ok, reason)| require(*ok, field, reason))?;
        block.preload.iter().try_for_each(|p| check_path(p))?;
        self.check_display()
    }

    fn check_display(&self) -> Result<()> {
        let shown = self.display.clone().unwrap_or_default();
        let pictured = matches!(self.kind, Kind::Video | Kind::Image);
        let framed = pictured || (shown.fit, shown.background.as_ref(), shown.flip) == (None, None, None);
        let spread = pictured
            || shown.span.is_none()
            || (self.kind == Kind::Scene && shown.span == Some(Span::AllOutputs));
        let moved = self.kind == Kind::Video || (shown.zoom.is_none() && shown.position.is_none());
        let accent = match &shown.accent {
            Some(Accent::Prop { prop }) => {
                self.properties.get(prop).is_some_and(|p| matches!(p.control, Control::Color { .. }))
            }
            _ => true,
        };
        let platforms = self.platforms.iter().collect::<HashSet<_>>().len() == self.platforms.len();
        let rules = [
            (
                "playback",
                matches!(self.kind, Kind::Video | Kind::Web) || self.playback.is_none(),
                "applies to video and web packages only",
            ),
            (
                "display",
                self.kind != Kind::Assets || self.display.is_none(),
                "does not apply to assets packages",
            ),
            ("display", framed, "fit, background and flip apply to pictures and videos only"),
            ("display.span", spread, "applies to pictures and videos, and all-outputs to scenes"),
            ("display", moved, "zoom and position apply to videos only"),
            ("display.accent", accent, "expected a #rrggbb color or {\"prop\": <id>} of a color property"),
            ("platforms", platforms, "names each of desktop and android once"),
        ];
        rules.iter().try_for_each(|(field, ok, reason)| require(*ok, field, reason))
    }

    pub fn dependencies(&self) -> &[Dependency] {
        self.requires.as_ref().map_or(&[], |r| &r.packages)
    }
}

impl PresetFile {
    pub fn parse(text: &[u8], manifest: &Manifest) -> Result<PresetFile> {
        let file: PresetFile = serde_json::from_slice(text)?;
        let invalid =
            |field: &str, reason: &str| Error::Invalid { field: field.into(), reason: reason.into() };
        let base = valid_id(&file.base.id) && VersionReq::parse(&file.base.version).is_ok();
        require(base, "preset.base", "expected the id of another package and a version requirement")?;
        let listed = manifest.dependencies().contains(&file.base);
        require(
            listed,
            "requires.packages",
            "lists the base of the preset with the same version requirement",
        )?;
        let ids = file.values.keys().chain(file.files.keys()).all(|id| valid_property(id));
        let count = file.values.len() + file.files.len() <= MAX_PRESET_VALUES;
        require(ids && count, "preset.values", "at most 256 values named like properties")?;
        file.files.values().try_for_each(|p| check_path(p))?;
        match file.files.keys().find(|id| file.values.contains_key(*id)) {
            Some(id) => Err(invalid(&format!("preset.files.{id}"), "is also in values")),
            None => Ok(file),
        }
    }
}

pub fn web_notice(exposes: &[String]) -> Option<wprs_core::message::Message> {
    use wprs_core::message::said;
    let named = |name: &String| match name.as_str() {
        "zone" => said("format.web_zone", "your time zone", &[]),
        "language" => said("format.web_language", "your language", &[]),
        "gpu" => said("format.web_gpu", "the model of your graphics card", &[]),
        "storage" => said("format.web_storage", "data it keeps on this computer", &[]),
        _ => said("format.web_network", "the network, to reach the sites it names", &[]),
    };
    let all = crate::features::listed(exposes.iter().map(named).collect())?;
    let text = "This web wallpaper uses %1. You can restrict each of them in its settings.";
    Some(said("format.web_notice", text, &[&all]).with(1, all))
}
