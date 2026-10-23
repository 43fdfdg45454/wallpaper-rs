use crate::{Error, Input, Kind, Manifest, Result, Scene, ensure, require};
use semver::{Prerelease, Version, VersionReq};
use std::collections::BTreeSet;
use wprs_scene::{Control, Uses, descriptor, needs, source, tells_time};

pub const REGISTRY: [(&str, &str); 7] = [
    ("inputs.audio", "reacts to the sound playing on the computer"),
    ("inputs.pointer", "follows the pointer"),
    ("inputs.clicks", "reacts to clicks"),
    ("inputs.media", "shows what the media player plays"),
    ("inputs.network", "connects to the network"),
    ("scripts", "runs scene scripts"),
    ("sound", "plays sound"),
];

pub const PROFILES: [(&str, &[&str]); 1] = [("android", &["scripts", "sound"])];

const DETECTED: [Input; 3] = [Input::Audio, Input::Pointer, Input::Clicks];

const EFFECTS: &str = "bloom blur blur_motion blur_radial chroma_key chromatic_aberration clouds color_adjust \
    color_lut crt depth_parallax dissolve edges film_grain flow glitch god_rays iris \
    lens_distort perspective pixelate posterize reflection scroll shake shine sway tilt_shift \
    twirl vignette water_rings water_ripple waves";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    pub features: BTreeSet<String>,
    pub inputs: BTreeSet<Input>,
    pub facets: Vec<String>,
    pub found: Vec<(Input, String)>,
}

fn builtin_first(id: &str) -> bool {
    EFFECTS.split_whitespace().any(|e| e == id)
}

pub fn known(name: &str) -> bool {
    let effect = |id: &str| !builtin_first(id) && descriptor(id).is_some();
    REGISTRY.iter().any(|(n, _)| *n == name) || name.strip_prefix("effect.").is_some_and(effect)
}

pub fn explain(name: &str) -> String {
    let found = REGISTRY.iter().find(|(n, _)| *n == name).map(|(_, why)| why.to_string());
    found.unwrap_or_else(|| format!("uses the {} effect", name.trim_start_matches("effect.")))
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
    let missing = PROFILES.iter().find(|(name, _)| *name == profile).map_or(&[][..], |(_, m)| *m);
    features.into_iter().filter(|f| missing.contains(&f.as_str())).cloned().collect()
}

pub fn facts(manifest: &Manifest, scene: Option<&Scene>, size: Option<[u32; 2]>) -> Facts {
    let uses = scene.map(needs).unwrap_or_default();
    let detected = [(uses.audio, Input::Audio), (uses.cursor, Input::Pointer), (uses.clicks, Input::Clicks)];
    let inputs: BTreeSet<Input> = detected.into_iter().filter_map(|(on, i)| on.then_some(i)).collect();
    let declared = manifest.inputs.iter().chain(&inputs).map(|i| format!("inputs.{}", i.name()));
    let sound = scene.is_some_and(|s| s.sounds().next().is_some());
    let effects = scene.into_iter().flat_map(|s| s.layers().flat_map(|l| &l.effects));
    let effects = effects.filter(|e| !builtin_first(&e.effect)).map(|e| format!("effect.{}", e.effect));
    let mut features: BTreeSet<String> = declared.chain(effects).collect();
    let used = [(uses.scripts, "scripts"), (sound, "sound")];
    features.extend(used.iter().filter(|(on, _)| *on).map(|(_, n)| n.to_string()));
    let author = manifest.inputs.iter().filter(|i| !DETECTED.contains(i));
    let mut facets: Vec<String> = inputs.iter().chain(author).map(|i| i.name().to_owned()).collect();
    let customizable =
        manifest.properties.values().any(|p| !matches!(p.control, Control::Group {} | Control::Label {}));
    let clock = scene.is_some_and(tells_time);
    let flags = [
        (sound, "sound"),
        (uses.scripts, "scripts"),
        (customizable, "customizable"),
        (clock, "clock"),
        (manifest.photosensitive == Some(true), "photosensitive"),
        (manifest.locales().len() > 1, "localized"),
        (manifest.kind == Kind::Scene || manifest.kind == Kind::Video, "animated"),
    ];
    facets.extend(flags.into_iter().filter(|(on, _)| *on).map(|(_, n)| n.to_owned()));
    let runs = PROFILES.iter().filter(|(name, _)| lacks(name, &features).is_empty());
    facets.extend(runs.map(|(name, _)| name.to_string()));
    let size = scene.map(|s| s.canvas.size).or(size);
    facets.extend(size.into_iter().flat_map(shape));
    facets.sort();
    facets.dedup();
    let missing = |i: &&Input| manifest.v2() && !manifest.inputs.contains(i);
    let found =
        scene.map_or(Vec::new(), |s| inputs.iter().filter(missing).map(|i| (*i, place(s, *i))).collect());
    Facts { features, inputs, facets, found }
}

fn place(scene: &Scene, input: Input) -> String {
    let pick: fn(&Uses) -> bool = match input {
        Input::Audio => |u| u.audio,
        Input::Clicks => |u| u.clicks,
        _ => |u| u.cursor,
    };
    source(scene, pick).unwrap_or_else(|| "/".into())
}

fn shape([w, h]: [u32; 2]) -> Vec<String> {
    const SIZES: [(&str, u32, u32); 6] = [
        ("8k", 7680, 4320),
        ("5k", 5120, 2880),
        ("4k", 3840, 2160),
        ("1440p", 2560, 1440),
        ("1080p", 1920, 1080),
        ("720p", 1280, 720),
    ];
    let (long, short) = (w.max(h), w.min(h));
    let orient = match w.cmp(&h) {
        std::cmp::Ordering::Greater => "landscape",
        std::cmp::Ordering::Less => "portrait",
        std::cmp::Ordering::Equal => "square",
    };
    let size = SIZES.iter().find(|(_, a, b)| long >= *a && short >= *b).map(|(n, ..)| *n);
    let wide = (long as f32 / short.max(1) as f32 >= 2.2).then_some("ultrawide");
    [Some(orient), size, wide].into_iter().flatten().map(String::from).collect()
}

pub fn check(manifest: &Manifest, facts: &Facts) -> Result<()> {
    if let Some(missing) = facts.inputs.iter().find(|i| !manifest.inputs.contains(i)) {
        let at = facts.found.iter().find(|(i, _)| i == missing).map_or("/", |(_, at)| at.as_str());
        let reason = format!("the scene uses {0} at {at} of scene.json; add \"{0}\"", missing.name());
        return Err(Error::Invalid { field: "inputs".into(), reason });
    }
    let listed = manifest.features();
    match facts.features.iter().find(|f| !listed.contains(f)) {
        Some(f) => Err(Error::Invalid {
            field: "requires.features".into(),
            reason: format!("add {f:?} ({}); pack build fills this list", explain(f)),
        }),
        None => Ok(()),
    }
}

pub fn warnings(manifest: &Manifest, facts: &Facts) -> Vec<String> {
    let unused = manifest.features().iter().filter(|f| !facts.features.contains(*f));
    let inputs = manifest.inputs.iter().filter(|i| !facts.inputs.contains(i) && DETECTED.contains(i));
    let inputs = inputs.map(|i| format!("inputs declares {} but nothing in the package uses it", i.name()));
    unused
        .map(|f| format!("requires.features lists {f}, which the package does not use"))
        .chain(inputs)
        .collect()
}
