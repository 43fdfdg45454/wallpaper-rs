use super::limb::boxed;
use crate::resolve::Context;
use crate::{Affine, Extent, Layer, MeshLayer};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Seat {
    pub to_mesh: Affine,
    pub cursor: Option<[f32; 2]>,
    pub size: [f32; 2],
}

pub(crate) fn size(cx: &Context, layer: &Layer, m: &MeshLayer) -> [f32; 2] {
    match cx.script.layers.get(&layer.id).and_then(|s| s.size) {
        Some(Extent::Box(b)) => b,
        _ => m.size.unwrap_or(cx.canvas),
    }
}

pub(crate) fn mount(cx: &Context, layer: &Layer, size: [f32; 2]) -> Affine {
    mount_on(cx, layer, size, cx.placement(layer).world)
}

pub(crate) fn mount_on(cx: &Context, layer: &Layer, size: [f32; 2], world: Affine) -> Affine {
    let set = cx.script.layers.get(&layer.id).unwrap_or(&crate::input::UNSET);
    let t = &layer.transform;
    let origin = set.origin.or(t.origin).or(layer.anchor.map(|a| a.fraction())).unwrap_or([0.5; 2]);
    let corner = Affine::translate([-origin[0] * size[0], -origin[1] * size[1]]);
    world.then(corner).then(boxed(size))
}

impl Seat {
    pub(crate) fn new(
        cx: &Context,
        layer: &Layer,
        m: &MeshLayer,
        pointer: Option<[f32; 2]>,
        placed: bool,
    ) -> Seat {
        let size = size(cx, layer, m);
        let to_mesh = if placed { mount(cx, layer, size).inverse() } else { Affine::IDENTITY };
        let v = cx.view;
        let cursor = pointer.map(|[x, y]| to_mesh.apply([v[0] + x * v[2], v[1] + y * v[3]]));
        Seat { to_mesh, cursor, size }
    }

    pub(crate) fn boxed_at(&self, [x, y]: [f32; 2]) -> [f32; 2] {
        [x - self.size[0] / 2.0, self.size[1] / 2.0 - y]
    }

    pub(crate) fn layer(&self, cx: &Context, id: &str) -> Option<[f32; 2]> {
        let w = cx.placement(cx.scene.layer(id)?).world;
        Some(self.to_mesh.apply([w.0[4], w.0[5]]))
    }
}
