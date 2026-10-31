use super::{Control, Grants, Input, View};
use crate::daemon::Daemon;
use crate::error::{Result, invalid, not_found};
use crate::streams::OutputInfo;
use std::collections::{BTreeMap, BTreeSet};

impl Control {
    pub(crate) fn granted(&self, scene: &str, key: &str) -> BTreeSet<Input> {
        Input::ALL.into_iter().filter(|input| self.has(scene, key, *input)).collect()
    }

    pub(crate) fn has(&self, scene: &str, key: &str, input: Input) -> bool {
        let keys = self.grants.get(scene).filter(|_| !key.is_empty());
        keys.and_then(|keys| keys.get(key)).is_some_and(|inputs| inputs.contains(&input))
    }

    pub(crate) fn allows(&self, output: &str, input: Input) -> bool {
        let still = input == Input::Pointer && self.parallax.contains(output);
        still
            || self.shown.get(output).is_some_and(|need| {
                let declared = match input {
                    Input::Audio => need.audio && !self.blocked,
                    Input::Pointer => need.pointer,
                    Input::Clicks => need.clicks && self.views.get(output).is_some_and(|v| v.clicks),
                    Input::Network => need.network,
                };
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

impl Daemon {
    pub(crate) fn views(&self) -> BTreeMap<String, View> {
        let st = self.state();
        let shown = |name: &String, info: &OutputInfo| {
            info.claims.contains_key(crate::streams::SURFACE) || self.exporter.is_streaming(name)
        };
        let view = |name: &String, info: &OutputInfo| View {
            active: st.active(info) && !st.locked,
            covered: info.covered() || !shown(name, info),
            rect: info.screen.as_ref().map(|s| [s.x.into(), s.y.into(), s.width.into(), s.height.into()]),
            clicks: st.config.clicks(name),
        };
        st.outputs.iter().map(|(name, info)| (name.clone(), view(name, info))).collect()
    }
}

pub(crate) fn reported(control: &Control) -> Grants {
    let reported = |(id, keys): (&String, &Grants)| {
        let shown: Vec<&str> = control
            .shown
            .values()
            .filter(|s| s.scene == *id && !s.key.is_empty())
            .map(|s| s.key.as_str())
            .collect();
        let keys = keys.iter().filter(|(key, _)| shown.is_empty() || shown.contains(&key.as_str()));
        let inputs: BTreeSet<Input> = keys.flat_map(|(_, inputs)| inputs).copied().collect();
        (!inputs.is_empty()).then(|| (id.clone(), inputs))
    };
    control.grants.iter().filter_map(reported).collect()
}
