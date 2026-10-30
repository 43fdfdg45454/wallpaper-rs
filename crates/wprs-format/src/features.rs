use crate::{Error, Facts, Manifest, Platform, Result, ensure, require};
use semver::{Prerelease, Version, VersionReq};
use wprs_scene::{described, drawn, newer_params};

pub use crate::facts::{BUILTIN, check, facts, warnings};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    Runs,
    Without,
    Refused,
}

use Support::{Refused as No, Runs as Yes, Without};

pub use crate::scenery::SCENE;

pub const REGISTRY: &[(&str, &str, [Support; 2])] = &[
    ("inputs.audio", "reacts to the sound playing on the computer", [Yes, Yes]),
    ("inputs.pointer", "follows the pointer", [Yes, Yes]),
    ("inputs.clicks", "reacts to clicks", [Yes, Yes]),
    ("inputs.media", "shows what the media player plays", [Yes, Yes]),
    ("inputs.network", "connects to the network", [Yes, Yes]),
    ("scripts", "runs scene scripts", [Yes, Yes]),
    ("sound", "plays sound", [Yes, Without]),
    ("properties2", "has settings of the newer kinds", [Yes, Yes]),
    ("folder", "shows pictures from a folder the user chooses", [Yes, Without]),
    ("user_videos", "plays videos the user chooses", [Yes, Without]),
    ("video_properties", "sets how its video plays", [Yes, Without]),
    ("web", "is a web page", [No, No]),
    ("preset", "sets the values of another package", [No, No]),
    ("assets", "holds resources for other packages", [No, No]),
    ("packages", "needs other packages", [No, No]),
    ("gpu", "says what graphics card it needs", [Without, Without]),
    ("platforms", "names the devices it is made for", [Yes, Yes]),
    ("locale_files", "keeps its texts in a file per language", [Yes, Yes]),
    ("engine_strings", "names texts the engine translates", [Yes, Yes]),
    ("newer_keys", "writes newer settings with their default values", [Yes, Yes]),
    ("origin2", "says that it was imported or where it was downloaded from", [Yes, Yes]),
    ("credits2", "credits files to the package they come from", [Yes, Yes]),
    ("display2", "sets the color around its picture or spreads over every screen", [Yes, Without]),
    ("accent", "suggests an accent color for the desktop", [Without, Without]),
    ("flip", "shows its picture or video mirrored", [No, No]),
    ("playback2", "limits its frame rate, starts muted or plays once", [No, No]),
    ("shortcut", "opens what the user chooses when clicked", [No, No]),
    ("folder2", "reads the subfolders of a chosen folder or fewer of its files", [Yes, Without]),
    ("folder_lists", "lets its scripts or page list a chosen folder", [No, No]),
    ("quality_settings", "changes its settings when the device saves power", [No, No]),
    ("ktx2", "uses compressed textures in the KTX2 format", [No, No]),
    ("avif", "uses pictures in the AVIF format", [No, No]),
    ("spanned_scenes", "spreads its scene over every screen", [No, Without]),
];

const RELEASED: &str = "inputs.audio inputs.pointer inputs.clicks inputs.media inputs.network scripts sound \
    properties2 folder user_videos video_properties";

pub(crate) fn released(name: &str) -> bool {
    RELEASED.split_whitespace().any(|n| n == name)
}

fn builtin_first(id: &str) -> bool {
    BUILTIN.split_whitespace().any(|e| e == id)
}

pub fn all() -> impl Iterator<Item = &'static (&'static str, &'static str, [Support; 2])> {
    REGISTRY.iter().chain(SCENE)
}

fn entry(name: &str) -> Option<&'static (&'static str, &'static str, [Support; 2])> {
    all().find(|(n, ..)| *n == name)
}

fn effect(name: &str) -> Option<(&str, Option<&str>)> {
    let rest = name.strip_prefix("effect.")?;
    let (id, param) = rest.split_once('.').map_or((rest, None), |(id, p)| (id, Some(p)));
    let known = match param {
        None => !builtin_first(id) && described(id).is_some(),
        Some(p) => builtin_first(id) && newer_params(id).any(|d| d.id == p),
    };
    known.then_some((id, param))
}

pub fn known(name: &str) -> bool {
    entry(name).is_some() || effect(name).is_some()
}

pub fn explain(name: &str) -> String {
    let found = entry(name).map(|(_, why, _)| why.to_string());
    found.unwrap_or_else(|| match effect(name) {
        Some((id, Some(param))) => format!("sets {param} on the {id} effect"),
        _ => format!("uses the {} effect", name.trim_start_matches("effect.")),
    })
}

pub const CHOSEN: [&str; 1] = ["locale_files"];

pub fn support(platform: Platform, name: &str) -> Support {
    let effect = effect(name).map(|(id, param)| if drawn(id, param) { Yes } else { No });
    entry(name).map_or(effect.unwrap_or(No), |(.., on)| on[platform as usize])
}

pub(crate) fn supported(engine: Option<&str>, features: &[String]) -> Result<()> {
    if let Some(engine) = engine {
        let wanted = VersionReq::parse(engine);
        require(wanted.is_ok(), "requires.engine", "expected a version requirement such as >=0.6")?;
        let mut version = Version::parse(wprs_core::VERSION).unwrap_or(Version::new(0, 0, 0));
        version.pre = Prerelease::EMPTY;
        let ok = wanted.as_ref().is_ok_and(|w| w.matches(&version));
        ensure(ok, || Error::Newer(format!("wallpaper-rs {engine}")))?;
    }
    features.iter().find(|f| !known(f)).map_or(Ok(()), |f| Err(Error::Newer(format!("the feature {f}"))))
}

pub fn lacks<'a>(profile: &str, features: impl IntoIterator<Item = &'a String>) -> Vec<String> {
    with(profile, Without, features)
}

pub fn refused<'a>(profile: &str, features: impl IntoIterator<Item = &'a String>) -> Vec<String> {
    with(profile, No, features)
}

fn with<'a>(profile: &str, kind: Support, features: impl IntoIterator<Item = &'a String>) -> Vec<String> {
    let Some(platform) = Platform::ALL.into_iter().find(|p| p.name() == profile) else { return Vec::new() };
    features.into_iter().filter(|f| support(platform, f) == kind).cloned().collect()
}

pub fn playable(manifest: &Manifest, facts: &Facts, platform: Platform) -> Result<()> {
    let allowed = manifest.platforms.is_empty() || manifest.platforms.contains(&platform);
    let names: Vec<&str> = manifest.platforms.iter().map(|p| p.title()).collect();
    ensure(allowed, || Error::Elsewhere(names.join(" and ")))?;
    let missing = refused(platform.name(), &facts.features);
    let named: Vec<String> = missing.iter().map(|f| format!("{f} ({})", explain(f))).collect();
    match named.as_slice() {
        [] => Ok(()),
        [one] => Err(Error::Newer(format!("the feature {one}"))),
        [rest @ .., last] => Err(Error::Newer(format!("the features {} and {last}", rest.join(", ")))),
    }
}
