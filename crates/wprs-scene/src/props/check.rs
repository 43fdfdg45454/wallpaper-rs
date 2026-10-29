use super::value::{MAX_TEXT, same};
use super::{Choice, Control, Preset, Property};
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};

pub const ICONS: &str = "appointment-soon audio-volume-high audio-volume-muted audio-x-generic \
    camera-photo dialog-information document-open edit-find emblem-favorite folder go-home \
    image-x-generic input-keyboard input-mouse media-playback-start media-playlist-repeat \
    media-playlist-shuffle preferences-desktop-theme preferences-system text-x-generic video-display \
    video-x-generic view-refresh weather-clear weather-clear-night weather-few-clouds weather-overcast \
    weather-showers weather-snow weather-storm x-office-calendar zoom-in";

fn need(ok: bool, reason: &str) -> Result<(), String> {
    ok.then_some(()).ok_or_else(|| reason.into())
}

fn scalar(value: &Json) -> bool {
    match value {
        Json::String(s) => !s.is_empty(),
        Json::Number(n) => n.as_f64().is_some_and(f64::is_finite),
        Json::Bool(_) => true,
        _ => false,
    }
}

pub fn icon(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

impl Property {
    pub fn check(&self) -> Result<(), String> {
        need(self.order.is_finite(), "order must be a finite number")?;
        let icons = self.icon.iter().map(String::as_str).chain(match &self.control {
            Control::Combo { options, .. } => options.iter().filter_map(Choice::icon).collect(),
            _ => Vec::new(),
        });
        need(icons.clone().all(icon), "icons are names of [a-z0-9-] such as weather-snow")?;
        match &self.control {
            Control::Slider(s) => {
                let finite = [s.min, s.max, s.default].iter().chain(&s.step).all(|v| v.is_finite());
                let step = s.step.is_none_or(|step| step > 0.0 && step <= s.max - s.min);
                need(
                    finite && s.min < s.max && step,
                    "expected finite values with min < max and 0 < step <= max - min",
                )?;
                need(s.decimals.is_none_or(|d| d <= 6), "decimals must be 0 to 6")?;
                need(!s.log || s.min > 0.0, "a logarithmic slider needs min > 0")?;
                let aligned = s.step.is_none_or(|step| {
                    let steps = (s.default - s.min) / step;
                    (steps - steps.round()).abs() <= 1e-9 * steps.max(1.0)
                });
                need((s.min..=s.max).contains(&s.default) && aligned, "default must lie on a step in range")?;
            }
            Control::Combo { options, .. } => {
                let kinds: BTreeSet<_> =
                    options.iter().map(|o| (o.value().is_string(), o.value().is_number())).collect();
                let unique = options.iter().enumerate().all(|(i, o)| {
                    scalar(o.value()) && !options[..i].iter().any(|p| same(p.value(), o.value()))
                });
                need(unique && kinds.len() == 1, "options must be non-empty, unique and of one type")?;
            }
            Control::Text { max_len: Some(max), .. } if *max as usize > MAX_TEXT => {
                return Err(format!("max_len must be at most {MAX_TEXT}"));
            }
            Control::Text { secret: true, default, .. } => {
                need(default.is_empty(), "a secret text has no default")?
            }
            Control::Folder { every, max_files, recursive, .. } => {
                need((10..=86_400).contains(every), "every must be 10 to 86400 seconds")?;
                need((1..=1000).contains(max_files), "max_files must be 1 to 1000")?;
                need(*recursive <= 2, "recursive must be 0 to 2 levels")?
            }
            _ => {}
        }
        let tuned = matches!(self.control, Control::Slider(_) | Control::Combo { .. } | Control::Bool { .. });
        need(tuned || self.quality.is_empty(), "only sliders, combos and flags change with quality")?;
        for (level, value) in &self.quality {
            self.value(value).map_err(|e| {
                format!(
                    "quality.{} {e}",
                    serde_json::to_value(level).unwrap_or_default().as_str().unwrap_or_default()
                )
            })?;
        }
        let valued = matches!(
            self.control,
            Control::Combo { .. }
                | Control::Color { .. }
                | Control::Text { .. }
                | Control::Slider(_)
                | Control::Bool { .. }
        );
        match valued {
            true => self.value(&self.default_value()).map(drop).map_err(|e| format!("default {e}")),
            false => Ok(()),
        }
    }
}

pub fn check_presets(props: &BTreeMap<String, Property>, presets: &[Preset]) -> Result<(), (String, String)> {
    for (id, prop) in props {
        let at = |key: &str| format!("properties.{id}.{key}");
        let group = |p: &Property| matches!(p.control, Control::Group { .. });
        if let Some(g) = &prop.group {
            need(props.get(g).is_some_and(group), &format!("{g} is not a group"))
                .map_err(|e| (at("group"), e))?;
            need(!group(prop), "a group cannot go into another group").map_err(|e| (at("group"), e))?;
        }
        prop.visible_if.check(props, id).map_err(|e| (at("visible_if"), e))?;
        prop.enabled_if.check(props, id).map_err(|e| (at("enabled_if"), e))?;
    }
    if let Some(id) = cycle(props) {
        return Err((format!("properties.{id}"), format!("the conditions of {id} depend on themselves")));
    }
    for (i, preset) in presets.iter().enumerate() {
        if presets[..i].iter().any(|p| p.id == preset.id) {
            return Err((format!("presets.{i}.id"), format!("preset {} is defined twice", preset.id)));
        }
        for (id, value) in &preset.values {
            let fail = |reason: String| {
                (format!("presets.{i}.values.{id}"), format!("preset {}: {reason}", preset.id))
            };
            let prop = props.get(id).ok_or_else(|| fail(format!("sets the unknown property {id}")))?;
            if prop.personal() {
                return Err(fail(format!("cannot set the {} property {id}", prop.kind())));
            }
            prop.value(value).map_err(|e| fail(format!("{id} {e}")))?;
        }
    }
    Ok(())
}

pub fn cycle(props: &BTreeMap<String, Property>) -> Option<&String> {
    let mut marks = BTreeMap::new();
    props.keys().find(|id| looped(props, id, &mut marks))
}

fn looped(props: &BTreeMap<String, Property>, id: &str, marks: &mut BTreeMap<String, bool>) -> bool {
    match marks.get(id) {
        Some(active) => return *active,
        None => drop(marks.insert(id.to_owned(), true)),
    }
    let prop = &props[id];
    let next: BTreeSet<String> = prop.visible_if.ids().into_iter().chain(prop.enabled_if.ids()).collect();
    let found = next.iter().filter(|n| props.contains_key(*n)).any(|n| looped(props, n, marks));
    marks.insert(id.to_owned(), false);
    found
}
