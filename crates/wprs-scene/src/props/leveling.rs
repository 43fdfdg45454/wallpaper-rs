use super::{Control, Property, Values};
use crate::Quality;
use serde_json::Value as Json;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Leveling(Vec<(String, bool, BTreeMap<Quality, Json>)>);

impl Leveling {
    pub fn of(props: &BTreeMap<String, Property>) -> Leveling {
        let tuned = props.iter().filter(|(_, p)| !p.quality.is_empty());
        Leveling(
            tuned
                .map(|(id, p)| (id.clone(), matches!(p.control, Control::Slider(_)), p.quality.clone()))
                .collect(),
        )
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn apply(&self, values: &mut Values, quality: Quality) {
        for (id, slider, levels) in &self.0 {
            let Some(value) = values.get_mut(id) else { continue };
            let mut held = levels.range(quality..).map(|(_, v)| v);
            if !slider {
                held.next().into_iter().for_each(|v| value.clone_from(v));
                continue;
            }
            let number = |v: &Json| v.as_f64().unwrap_or(f64::INFINITY);
            let cap = held.min_by(|a, b| number(a).total_cmp(&number(b)));
            if let Some(cap) = cap.filter(|c| number(c) < number(value)) {
                value.clone_from(cap);
            }
        }
    }

    pub fn tuned(&self, values: &Values, quality: Quality) -> Values {
        let mut out = values.clone();
        self.apply(&mut out, quality);
        out
    }
}
