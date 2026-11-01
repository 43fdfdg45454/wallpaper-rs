use crate::input::LayerOverride;
use crate::resolve::Context;
use crate::{Affine, Inherit, Kind, Layer, Lean, MAX_DEPTH, Scene, Wake};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Place {
    pub(crate) world: Affine,
    pub(crate) lean: Option<Lean>,
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
        gyro && self.any_layer(|l| !l.parallax.is_zero())
    }
}

pub(crate) fn parent_of<'a>(layer: &'a Layer, set: &'a LayerOverride) -> Option<&'a String> {
    match &set.parent {
        Some(p) if p.is_empty() => None,
        Some(p) => Some(p),
        None => layer.parent.as_ref(),
    }
}

pub(crate) fn glide(scene: &Scene, script: &crate::ScriptOutputs) -> crate::look::Glide {
    let mut glide = scene.canvas.parallax.clone().unwrap_or_default();
    if let Some(set) = script.stage.and_then(|s| s.parallax) {
        glide.amount = set.amount.unwrap_or(glide.amount);
        glide.delay = set.delay.unwrap_or(glide.delay);
    }
    glide
}

fn follows(layer: &Layer, set: &LayerOverride, what: Inherit) -> bool {
    let default = matches!(what, Inherit::Transform | Inherit::Visible);
    parent_of(layer, set).is_some() && layer.inherit.as_ref().map_or(default, |list| list.contains(&what))
}

impl Context<'_> {
    pub(crate) fn place(&self, layer: &Layer, outer: (Affine, Option<Lean>)) -> Place {
        self.placed(layer, Some(outer), 0)
    }

    pub(crate) fn placement(&self, layer: &Layer) -> Place {
        self.placed(layer, None, 0)
    }

    pub(crate) fn world(&self, layer: &Layer) -> Affine {
        self.camera.inverse().then(self.placed(layer, None, 0).world)
    }

    pub(crate) fn placed(&self, layer: &Layer, outer: Option<(Affine, Option<Lean>)>, depth: usize) -> Place {
        let key = std::ptr::from_ref(layer) as usize;
        if let Some(found) = self.places.borrow().get(&key) {
            return *found;
        }
        let set = self.script.layers.get(&layer.id).unwrap_or(&crate::input::UNSET);
        let by_id = |id: &String| {
            let found = self.scene.layer(id).filter(|_| depth < 2 * MAX_DEPTH);
            found.map(|l| self.placed(l, None, depth + 1))
        };
        let parent = parent_of(layer, set).and_then(by_id);
        let home = || self.homes.get(&layer.id).and_then(by_id).map(|p| (p.world, p.lean));
        let follows = |what| follows(layer, set, what);
        let pinned = layer.attach.as_ref().filter(|_| set.parent.is_none());
        let pinned = pinned.and_then(|a| self.pinned(a, depth));
        let (base, around) = match parent.filter(|_| follows(Inherit::Transform)) {
            Some(p) => (p.world, p.lean),
            None => {
                pinned.map(|(w, lean, _)| (w, lean)).or(outer).or_else(home).unwrap_or((self.camera, None))
            }
        };
        let mut visible = set.visible.unwrap_or_else(|| layer.visible.get(self));
        let mut opacity = set.opacity.unwrap_or_else(|| layer.opacity.get(self)).clamp(0.0, 1.0);
        visible &= pinned.is_none_or(|p| p.2);
        if let Some(p) = parent {
            visible &= p.visible || !follows(Inherit::Visible);
            opacity *= if follows(Inherit::Opacity) { p.opacity } else { 1.0 };
        }
        let world = base.then(self.local(layer, set));
        let place = Place { world, lean: self.lean(layer, world, around), visible, opacity };
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
            || (layer.attach.is_some() && set.parent.is_none())
            || follows(layer, set, Inherit::Transform)
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
        let glide = glide(self.scene, self.script);
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
