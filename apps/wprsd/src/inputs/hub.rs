use super::{Bound, Control, Grants, Hub, View, Wake};
use crate::compositor::lock;
use crate::error::{Result, invalid, not_found};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Mutex;
use tokio::sync::watch;
use wprs_audio::Capture;
use wprs_core::model::Input;
use wprs_scene::PointerState;

impl View {
    pub(super) fn place(&self, x: f64, y: f64) -> Option<[f32; 2]> {
        let [left, top, width, height] = self.rect.filter(|_| self.active && !self.covered)?;
        let at = [(x - left) / width, (y - top) / height];
        at.iter().all(|v| (0.0..1.0).contains(v)).then(|| at.map(|v| v as f32))
    }
}

impl Control {
    pub(crate) fn granted(&self, scene: &str, key: &str) -> BTreeSet<Input> {
        [Input::Audio, Input::Pointer].into_iter().filter(|input| self.has(scene, key, *input)).collect()
    }

    pub(crate) fn has(&self, scene: &str, key: &str, input: Input) -> bool {
        let keys = self.grants.get(scene).filter(|_| !key.is_empty());
        keys.and_then(|keys| keys.get(key)).is_some_and(|inputs| inputs.contains(&input))
    }

    pub(crate) fn allows(&self, output: &str, input: Input) -> bool {
        let still = input == Input::Pointer && self.parallax.contains(output);
        still
            || self.shown.get(output).is_some_and(|need| {
                let declared = if input == Input::Audio { need.audio } else { need.pointer };
                declared && self.has(&need.scene, &need.key, input)
            })
    }

    pub(crate) fn package(&self, scene: &str, key: Option<&str>) -> Result<(String, String)> {
        let shown = self.shown.values().filter(|s| s.scene == scene && !s.key.is_empty());
        let found: BTreeMap<_, _> =
            shown.filter(|s| key.is_none_or(|k| k == s.key)).map(|s| (s.key.as_str(), s)).collect();
        match found.values().collect::<Vec<_>>()[..] {
            [s] => Ok((s.key.clone(), if s.title.is_empty() { s.scene.clone() } else { s.title.clone() })),
            [] => Err(not_found(format!(
                "{scene} is not shown; inputs are granted to the scene package that is shown"
            ))),
            _ => Err(invalid(format!(
                "more than one package with the id {scene} is shown; grant the input to one of them with GrantPackage"
            ))),
        }
    }

    pub(crate) fn wants(&self, input: Input) -> BTreeSet<String> {
        let wanted = |output: &&String| {
            let view = self.views.get(*output).copied().unwrap_or_default();
            self.allows(output, input) && view.active && !view.covered
        };
        self.shown.keys().chain(&self.parallax).filter(wanted).cloned().collect()
    }
}

impl Hub {
    pub(crate) fn new((grants, declined): (Bound, Grants), wake: Wake, changed: Wake) -> Hub {
        let control = Mutex::new(Control { grants, declined, ..Control::default() });
        let tracking = watch::Sender::new(false);
        let (switch, audio, pointers, moved, poke) = Default::default();
        Hub { control, tracking, switch, audio, pointers, moved, poke, wake, changed }
    }

    pub(crate) fn grant(&self, path: &Path, scene: &str, key: &str, input: Input) -> anyhow::Result<()> {
        self.decide(path, scene, Some(key), (input, true))
    }

    pub(crate) fn decline(&self, path: &Path, scene: &str, key: &str, input: Input) -> anyhow::Result<()> {
        self.decide(path, scene, Some(key), (input, false))
    }

    pub(crate) fn revoke(&self, path: &Path, scene: &str, input: Input) -> anyhow::Result<()> {
        self.decide(path, scene, None, (input, false))
    }

    pub(crate) fn changed(&self) {
        (self.changed)()
    }

    fn decide(
        &self,
        path: &Path,
        scene: &str,
        key: Option<&str>,
        (input, granted): (Input, bool),
    ) -> anyhow::Result<()> {
        let switch = lock(&self.switch);
        let (mut grants, mut declined) = {
            let control = lock(&self.control);
            if let Some(key) = key.filter(|_| granted) {
                control.package(scene, Some(key))?;
            }
            (control.grants.clone(), control.declined.clone())
        };
        let bound = grants.entry(scene.to_owned()).or_default();
        let changed = match key {
            Some(key) => toggle(bound, key, input, granted),
            None => bound.clone().keys().fold(false, |c, key| toggle(bound, key, input, false) | c),
        };
        grants.retain(|_, keys| !keys.is_empty());
        let decided = toggle(&mut declined, scene, input, !granted);
        if changed {
            super::save(path, &grants)?;
        }
        if decided {
            super::save(&super::declined_path(path), &declined)?;
        }
        if !changed && !decided {
            return Ok(());
        }
        let mut control = lock(&self.control);
        (control.grants, control.declined) = (grants, declined);
        drop((control, switch));
        tracing::info!(scene, input = input.as_str(), granted, "input permission changed");
        self.update(None);
        (self.changed)();
        Ok(())
    }

    pub(crate) fn update(&self, views: Option<BTreeMap<String, View>>) {
        let mut switch = lock(&self.switch);
        let mut control = lock(&self.control);
        if let Some(views) = views {
            control.views = views;
        }
        let (audio, followed) = (!control.wants(Input::Audio).is_empty(), control.wants(Input::Pointer));
        let flips: Vec<_> = (control.windows.iter())
            .filter(|(output, _)| control.followed.contains(*output) != followed.contains(*output))
            .map(|(output, window)| (window.clone(), followed.contains(output)))
            .collect();
        let refollowed = std::mem::replace(&mut control.followed, followed.clone()) != followed;
        drop(control);
        flips.into_iter().for_each(|(window, on)| window.track_pointer(on));
        if refollowed {
            lock(&self.pointers).retain(|output, _| followed.contains(output));
            let on = !followed.is_empty();
            self.tracking.send_if_modified(|was| std::mem::replace(was, on) != on);
            tracing::info!(outputs = followed.len(), "pointer feed");
        }
        let switched = audio != switch.audio;
        if switched {
            lock(&self.audio).attach(None);
            drop(switch.capture.take());
            let wake = self.wake.clone();
            let started = audio.then(|| Capture::start(move || wake())).transpose();
            let started = started.inspect_err(|e| tracing::warn!("cannot capture audio: {e}"));
            let (capture, feed) = started.ok().flatten().unzip();
            lock(&self.audio).attach(feed);
            (switch.audio, switch.capture) = (audio, capture);
            lock(&self.control).capturing = switch.capture.is_some();
            tracing::info!(on = switch.capture.is_some(), "audio capture");
        }
        drop(switch);
        if refollowed || switched {
            (self.changed)()
        }
    }

    pub(crate) fn set_pointer(&self, x: f64, y: f64) {
        let control = lock(&self.control);
        let followed = control.views.iter().filter(|(output, _)| control.followed.contains(*output));
        let place = |(output, view): (&String, &View)| {
            let pos = view.place(x, y);
            (output.clone(), PointerState { pos, inside: pos.is_some(), ..PointerState::default() })
        };
        let states: Vec<_> = followed.map(place).collect();
        drop(control);
        self.store(states);
    }

    pub(super) fn store(&self, states: impl IntoIterator<Item = (String, PointerState)>) {
        let control = lock(&self.control);
        let (mut pointers, mut moved) = (lock(&self.pointers), false);
        for (output, state) in states.into_iter().filter(|(output, _)| control.followed.contains(output)) {
            moved |= pointers.insert(output, state) != Some(state);
        }
        if moved {
            self.moved.notify_one();
        }
    }
}

fn toggle(map: &mut Grants, scene: &str, input: Input, on: bool) -> bool {
    let inputs = map.entry(scene.to_owned()).or_default();
    let changed = if on { inputs.insert(input) } else { inputs.remove(&input) };
    map.retain(|_, inputs| !inputs.is_empty());
    changed
}
