use crate::input::LayerOverride;
use crate::resolve::Context;
use crate::{Affine, Inherit, Kind, Layer, MAX_DEPTH, Scene, Wake};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Place {
    pub(crate) world: Affine,
    pub(crate) visible: bool,
    pub(crate) opacity: f32,
}

pub(crate) fn homes(scene: &Scene) -> HashMap<String, String> {
    let mut found = HashMap::new();
    for group in scene.layers() {
        found.extend(group.children().iter().map(|l| (l.id.clone(), group.id.clone())));
    }
    found
}

impl Scene {
    pub fn tilts(&self) -> bool {
        let gyro = self.canvas.parallax.as_ref().is_some_and(|g| g.gyro);
        gyro && self.layers().any(|l| !l.parallax.is_zero())
    }
}

fn follows(layer: &Layer, what: Inherit) -> bool {
    let default = matches!(what, Inherit::Transform | Inherit::Visible);
    layer.parent.is_some() && layer.inherit.as_ref().map_or(default, |list| list.contains(&what))
}

impl Context<'_> {
    pub(crate) fn place(&self, layer: &Layer, outer: Affine) -> Place {
        self.placed(layer, Some(outer), 0)
    }

    fn placed(&self, layer: &Layer, outer: Option<Affine>, depth: usize) -> Place {
        let key = std::ptr::from_ref(layer) as usize;
        if let Some(found) = self.places.borrow().get(&key) {
            return *found;
        }
        let set = self.script.layers.get(&layer.id).unwrap_or(&crate::input::UNSET);
        let by_id = |id: &String| {
            let found = self.scene.layer(id).filter(|_| depth < 2 * MAX_DEPTH);
            found.map(|l| self.placed(l, None, depth + 1))
        };
        let parent = layer.parent.as_ref().and_then(by_id);
        let home = || self.homes.get(&layer.id).and_then(by_id).map(|p| p.world);
        let base = match parent.filter(|_| follows(layer, Inherit::Transform)) {
            Some(p) => p.world,
            None => outer.or_else(home).unwrap_or(Affine::IDENTITY),
        };
        let mut visible = set.visible.unwrap_or_else(|| layer.visible.get(self));
        let mut opacity = set.opacity.unwrap_or_else(|| layer.opacity.get(self)).clamp(0.0, 1.0);
        if let Some(p) = parent {
            visible &= p.visible || !follows(layer, Inherit::Visible);
            opacity *= if follows(layer, Inherit::Opacity) { p.opacity } else { 1.0 };
        }
        let place = Place { world: base.then(self.local(layer, set)), visible, opacity };
        self.places.borrow_mut().insert(key, place);
        place
    }

    fn local(&self, layer: &Layer, set: &LayerOverride) -> Affine {
        let (t, view, canvas) = (&layer.transform, self.view, self.canvas);
        let base = layer.anchor.map_or([0.0; 2], |a| {
            let [fx, fy] = a.fraction();
            [view[0] + fx * view[2], view[1] + fy * view[3]]
        });
        let free = layer.anchor.is_some()
            || follows(layer, Inherit::Transform)
            || matches!(layer.kind, Kind::Group { .. } | Kind::Adjustment { .. });
        let center = if free { [0.0; 2] } else { [canvas[0] / 2.0, canvas[1] / 2.0] };
        let position = set.position.unwrap_or_else(|| t.position.as_ref().map_or(center, |p| p.get(self).0));
        let shift = self.shift(set.parallax.unwrap_or(layer.parallax.0));
        let at = [0, 1].map(|i| base[i] + position[i] + shift[i] * canvas[i]);
        Affine::translate(at)
            .then(Affine::rotate(set.rotation.unwrap_or_else(|| t.rotation.get(self))))
            .then(Affine::skew(set.skew.unwrap_or_else(|| t.skew.get(self).0)))
            .then(Affine::scale(set.scale.unwrap_or_else(|| t.scale.get(self).0)))
    }

    fn shift(&self, depth: [f32; 2]) -> [f32; 2] {
        if depth == [0.0; 2] || self.still {
            return [0.0; 2];
        }
        let glide = self.scene.canvas.parallax.clone().unwrap_or_default();
        let tilt = if glide.gyro { self.tilt } else { [0.0; 2] };
        if glide.pointer > 0.0 && self.glide.moving() {
            self.note(Wake::Frame);
        }
        [0, 1].map(|i| {
            let offset = (self.glide.smooth[i] - 0.5) * glide.pointer + tilt[i] * 0.5;
            offset * glide.amount * depth[i]
        })
    }
}
