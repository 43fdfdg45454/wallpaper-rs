use super::super::power::Power;
use super::super::state::State;
use super::PageSlot;
use crate::pages::{PageKey, PageSpec};
use std::collections::{BTreeMap, HashMap};

pub(super) type Views = Vec<(String, (u32, u32), (bool, bool), u32, PageSpec)>;
pub(super) type Wanted = BTreeMap<PageKey, (PageSpec, Views)>;

pub(in crate::compositor) fn level(power: Power, spec: &PageSpec) -> wprs_scene::Quality {
    Power { choice: spec.quality.unwrap_or(power.choice), ..power }.steady()
}

pub(super) fn rekey(pages: &mut HashMap<PageKey, PageSlot>, wanted: &Wanted) {
    let orphans: Vec<PageKey> = pages.keys().filter(|k| !wanted.contains_key(*k)).cloned().collect();
    for old in orphans {
        let slot = pages.remove(&old).expect("listed above");
        let free = wanted.keys().find(|k| k.0 == old.0 && !pages.contains_key(*k));
        if let Some(new) = free {
            pages.insert(new.clone(), slot);
        }
    }
}

impl State {
    pub(super) fn wanted(&self) -> Wanted {
        let mut wanted = Wanted::new();
        for (name, o) in &self.outputs {
            let (visible, fps) = o.page_view(name, self.exporter.is_streaming(name), self.power);
            for spec in o.layers().filter_map(|l| l.content.page()) {
                let key = spec.key(self.power.choice);
                let entry = wanted.entry(key).or_insert_with(|| (spec.clone(), Vec::new()));
                entry.1.push((name.clone(), o.size, visible, fps, spec.clone()));
            }
        }
        wanted
    }
}
