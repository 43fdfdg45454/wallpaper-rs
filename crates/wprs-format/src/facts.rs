use crate::features::{lacks, refused};
use crate::validate::extension;
use crate::{Input, Kind, Manifest, Platform, Result, Scene, Span};
use std::collections::BTreeSet;
use wprs_core::message::{Message, said};
use wprs_scene::{EffectUse, Uses, needs, newer_params, source, tells_time};

pub const BUILTIN: &str = "bloom blur blur_motion blur_radial chroma_key chromatic_aberration clouds \
    color_adjust color_lut crt depth_parallax dissolve edges film_grain flow glitch god_rays iris \
    lens_distort perspective pixelate posterize reflection scroll shake shine sway tilt_shift \
    twirl vignette water_rings water_ripple waves";

const CHECKED: [Input; 5] = [Input::Audio, Input::Pointer, Input::Clicks, Input::Media, Input::Network];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    pub features: BTreeSet<String>,
    pub inputs: BTreeSet<Input>,
    pub facets: Vec<String>,
    pub found: Vec<(Input, String)>,
    pub notes: Vec<String>,
}

fn builtin_first(id: &str) -> bool {
    BUILTIN.split_whitespace().any(|e| e == id)
}

fn effect_names(e: &EffectUse) -> Vec<String> {
    if !builtin_first(&e.effect) {
        return vec![format!("effect.{}", e.effect)];
    }
    let newer = e.params.keys().filter(|p| newer_params(&e.effect).any(|d| &&d.id == p));
    newer.map(|p| format!("effect.{}.{p}", e.effect)).collect()
}

fn declared(m: &Manifest, scene: Option<&Scene>) -> Vec<&'static str> {
    let display = m.display.clone().unwrap_or_default();
    let playback = m.playback.clone().unwrap_or_default();
    let requires = m.requires.clone().unwrap_or_default();
    let image = m.kind == Kind::Image && m.display.is_some();
    let pictured = matches!(m.kind, Kind::Image | Kind::Video);
    let web = m.kind == Kind::Web && m.playback.is_some();
    let video = m.kind == Kind::Video && (m.playback.is_some() || m.display.is_some());
    let paths = scene.into_iter().flat_map(|s| s.paths()).chain([m.entry.as_str()]);
    let files: BTreeSet<String> = paths.filter_map(extension).collect();
    [
        (m.kind == Kind::Web, "web"),
        (m.kind == Kind::Preset, "preset"),
        (m.kind == Kind::Assets, "assets"),
        (!requires.packages.is_empty(), "packages"),
        (requires.gpu.is_some(), "gpu"),
        (!m.platforms.is_empty(), "platforms"),
        (!m.string_files.is_empty(), "locale_files"),
        (m.texts().iter().any(|(_, t)| m.key(t).is_some_and(|k| k.starts_with("wprs."))), "engine_strings"),
        (m.origin.as_ref().is_some_and(|o| o.newer()), "origin2"),
        (m.credits.iter().any(|c| c.source.is_some()), "credits2"),
        (image || (pictured && display.newer()), "display2"),
        (display.accent.is_some(), "accent"),
        (display.flipped(), "flip"),
        (web || playback.newer() || (m.kind == Kind::Scene && m.playback.is_some()), "playback2"),
        (video, "video_properties"),
        (files.contains("ktx2"), "ktx2"),
        (files.contains("avif"), "avif"),
        (m.kind == Kind::Scene && display.span == Some(Span::AllOutputs), "spanned_scenes"),
        (m.permissions.as_ref().is_some_and(|p| p.rgb), "rgb"),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect()
}

pub fn facts(manifest: &Manifest, scene: Option<&Scene>, size: Option<[u32; 2]>) -> Facts {
    let uses = scene.map(needs).unwrap_or_default();
    let detected = [
        (uses.audio, Input::Audio),
        (uses.cursor, Input::Pointer),
        (uses.clicks, Input::Clicks),
        (uses.media, Input::Media),
    ];
    let inputs: BTreeSet<Input> = detected.into_iter().filter_map(|(on, i)| on.then_some(i)).collect();
    let named = manifest.inputs.iter().chain(&inputs).map(|i| format!("inputs.{}", i.name()));
    let sound = scene.is_some_and(|s| s.sounds().next().is_some());
    let layers = scene.into_iter().flat_map(|s| s.layers().chain(s.templates.values()).map(move |l| (s, l)));
    let effects = layers.flat_map(|(s, l)| l.effects.iter().map(move |e| (s, e)));
    let effects =
        effects.filter(|(s, e)| !s.effects.contains_key(&e.effect)).flat_map(|(_, e)| effect_names(e));
    let mut features: BTreeSet<String> = named.chain(effects).collect();
    let used = [(uses.scripts, "scripts"), (sound, "sound")];
    features.extend(used.iter().filter(|(on, _)| *on).map(|(_, n)| n.to_string()));
    features.extend(declared(manifest, scene).into_iter().map(String::from));
    features.extend(scene.into_iter().flat_map(Scene::features).map(String::from));
    features.extend(manifest.properties.values().flat_map(|p| p.features()).map(String::from));
    if manifest.kind != Kind::Web && features.remove("folder_lists") {
        features.insert("script_folders".into());
    }
    let page = manifest.kind == Kind::Web;
    let author = manifest.inputs.iter().filter(|i| page || !CHECKED.contains(i));
    let mut facets: Vec<String> = inputs.iter().chain(author).map(|i| i.name().to_owned()).collect();
    let video = manifest.playback.is_some() || manifest.display.is_some();
    let customizable = video || manifest.properties.values().any(|p| p.holds_value() || p.kind() == "button");
    let flags = [
        (sound, "sound"),
        (uses.scripts, "scripts"),
        (customizable, "customizable"),
        (scene.is_some_and(tells_time), "clock"),
        (manifest.photosensitive == Some(true), "photosensitive"),
        (manifest.locales().len() > 1, "localized"),
        (matches!(manifest.kind, Kind::Scene | Kind::Video | Kind::Web), "animated"),
        (manifest.web.as_ref().is_some_and(|w| w.storage_kib > 0), "storage"),
        (manifest.requires.as_ref().is_some_and(|r| r.gpu == Some(crate::Gpu::Compute)), "compute"),
    ];
    facets.extend(flags.into_iter().filter(|(on, _)| *on).map(|(_, n)| n.to_owned()));
    let allowed = |p: &Platform| manifest.platforms.is_empty() || manifest.platforms.contains(p);
    let fully =
        |p: &Platform| lacks(p.name(), &features).is_empty() && refused(p.name(), &features).is_empty();
    let runs = Platform::ALL.into_iter().filter(|p| allowed(p) && fully(p) && manifest.kind.shown());
    facets.extend(runs.map(|p| p.name().to_owned()));
    let size = scene.filter(|_| manifest.kind == Kind::Scene).map(|s| s.canvas.size).or(size);
    facets.extend(size.into_iter().flat_map(crate::shape::shape));
    facets.sort();
    facets.dedup();
    let missing = |i: &&Input| manifest.v2() && !manifest.inputs.contains(i);
    let found =
        scene.map_or(Vec::new(), |s| inputs.iter().filter(missing).map(|i| (*i, place(s, *i))).collect());
    let notes = scene.and_then(Scene::heavy).into_iter().collect();
    Facts { features, inputs, facets, found, notes }
}

fn place(scene: &Scene, input: Input) -> String {
    let pick: fn(&Uses) -> bool = match input {
        Input::Audio => |u| u.audio,
        Input::Clicks => |u| u.clicks,
        Input::Media => |u| u.media,
        _ => |u| u.cursor,
    };
    source(scene, pick).unwrap_or_else(|| "/".into())
}

pub fn check(manifest: &Manifest, facts: &Facts) -> Result<()> {
    let granted = manifest.permissions.as_ref().is_some_and(|p| p.rgb);
    if facts.features.contains("rgb") && !granted {
        let reason = said("format.rgb", "the scene sends its colors to RGB lighting; add \"rgb\": true", &[]);
        return Err(crate::refuse("permissions", reason));
    }
    if let Some(missing) = facts.inputs.iter().find(|i| !manifest.inputs.contains(i)) {
        let at = facts.found.iter().find(|(i, _)| i == missing).map_or("/", |(_, at)| at.as_str());
        let name = missing.name();
        let reason =
            said("format.undeclared", "the scene uses %1 at %2 of scene.json; add \"%1\"", &[&name, &at]);
        return Err(crate::refuse("inputs", reason));
    }
    let listed = manifest.features();
    match facts.features.iter().find(|f| !listed.contains(f)) {
        Some(f) => {
            let why = crate::features::explained(f);
            let reason = said("format.unlisted", "add \"%1\" (%2); pack build fills this list", &[f, &why]);
            Err(crate::refuse("requires.features", reason.with(2, why)))
        }
        None => Ok(()),
    }
}

pub fn notes(manifest: &Manifest, facts: &Facts) -> Vec<Message> {
    let unused = manifest.features().iter().filter(|f| !facts.features.contains(*f));
    let inputs = manifest.inputs.iter().filter(|i| !facts.inputs.contains(i) && CHECKED.contains(i));
    let inputs = inputs.filter(|_| manifest.kind != Kind::Web);
    let inputs = inputs.map(|i| {
        said("format.unused_input", "inputs declares %1 but nothing in the package uses it", &[&i.name()])
    });
    unused
        .map(|f| {
            said("format.unused_feature", "requires.features lists %1, which the package does not use", &[f])
        })
        .chain(inputs)
        .chain(facts.notes.iter().map(|n| said("format.note", "%1", &[n])))
        .collect()
}

pub fn warnings(manifest: &Manifest, facts: &Facts) -> Vec<String> {
    notes(manifest, facts).iter().map(ToString::to_string).collect()
}
