use crate::footage::owned;
use crate::{Control, Kind, Layer, Property, Scene, Value};
use std::collections::BTreeMap;

fn picked(layer: &Layer) -> bool {
    matches!(&layer.kind, Kind::Video { video: Value::Bound(b), .. } if b.map.is_none())
}

pub fn lane(scene: &Scene, layer: &Layer, asset: &str) -> Option<String> {
    let own = owned(scene, &layer.kind).into_iter().find(|id| picked(layer) || *id == asset)?;
    Some(format!("{own}@{}", layer.id))
}

impl Scene {
    pub fn lanes(&self) -> Vec<(String, String)> {
        let own = self.layers().flat_map(|l| owned(self, &l.kind).into_iter().map(move |a| (l, a)));
        own.map(|(l, asset)| (format!("{asset}@{}", l.id), asset.clone())).collect()
    }

    pub fn played_lanes(&self, properties: &BTreeMap<String, Property>) -> Vec<(String, String)> {
        let default = |l: &Layer, id: &String| match properties.get(id).map(|p| &p.control) {
            Some(Control::File { default_asset: Some(asset), .. }) if picked(l) => Some(asset.clone()),
            _ => None,
        };
        let own = self.layers().flat_map(|l| owned(self, &l.kind).into_iter().map(move |a| (l, a)));
        own.map(|(l, a)| (format!("{a}@{}", l.id), default(l, a).unwrap_or_else(|| a.clone()))).collect()
    }
}
