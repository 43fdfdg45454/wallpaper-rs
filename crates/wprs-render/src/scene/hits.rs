use super::paint::Painter;
use super::{Pixels, SceneView};
use std::collections::HashMap;
use std::sync::Arc;
use wprs_scene::{Affine, AlphaMap, Content, Hitbox, Hitboxes, Solid};

const EDGE: u32 = 128;

type Covers = HashMap<String, Option<Arc<AlphaMap>>>;
type Named<'n> = (&'n str, Solid, Option<Affine>);
type Sources = HashMap<String, Arc<Pixels>>;

fn coverage(pixels: &Pixels) -> Option<Arc<AlphaMap>> {
    let image = pixels.image()?;
    let (w, h) = image.dimensions();
    let size = [w.clamp(1, EDGE), h.clamp(1, EDGE)];
    let alpha = (0..size[0] * size[1])
        .map(|i| image.get_pixel((i % size[0]) * w / size[0], (i / size[0]) * h / size[1])[3])
        .collect();
    Some(Arc::new(AlphaMap { size, alpha }))
}

impl Painter<'_, '_, '_> {
    fn hitbox(
        &self,
        i: usize,
        (layer, solid, frame): Named,
        (sources, covers): (&Sources, &mut Covers),
        (out, n): (&mut Vec<Hitbox>, &mut usize),
    ) {
        let d = &self.frame.draws[i];
        let Some((size, _)) = self.geometry(i).filter(|_| !d.hidden) else { return };
        let place = d.world.then(Affine::translate([-d.origin[0] * size[0], -d.origin[1] * size[1]]));
        let to_box = Affine::scale([1.0 / size[0].max(1e-3), 1.0 / size[1].max(1e-3)]).then(place.inverse());
        let threshold = match solid {
            Solid::Hit { alpha } => Some(alpha),
            Solid::Flag(_) => None,
        };
        let (shade, asset) = match &d.content {
            Content::Solid { color, .. } | Content::Spectrum { color, .. } | Content::Text { color, .. } => {
                (color.0[3], None)
            }
            Content::Shape { fill, .. } => (fill.map_or(1.0, |c| c.0[3]), None),
            Content::Image { asset, .. } => (1.0, Some(asset.as_ref())),
            _ => (1.0, None),
        };
        let made = |a: &str| sources.get(a).and_then(|p| coverage(p));
        let asset = asset.filter(|_| threshold.is_some());
        let coverage = asset.and_then(|a| covers.entry(a.to_owned()).or_insert_with(|| made(a)).clone());
        let made = Hitbox {
            layer: String::new(),
            to_box,
            to_local: frame.unwrap_or(place).inverse(),
            cell: self.cell(i).0,
            alpha: d.opacity * d.tint.0[3] * shade,
            threshold,
            coverage,
        };
        match out.get_mut(*n) {
            Some(slot) => {
                let mut name = std::mem::take(&mut slot.layer);
                name.clear();
                name.push_str(layer);
                *slot = Hitbox { layer: name, ..made };
            }
            None => out.push(Hitbox { layer: layer.to_owned(), ..made }),
        }
        *n += 1;
    }

    pub(crate) fn hitboxes(&self, sources: &Sources, covers: &mut Covers, boxes: &mut Hitboxes) {
        let (draws, mut n) = (&self.frame.draws, 0);
        for (i, d) in draws.iter().enumerate().filter(|(_, d)| !d.layer.solid.is_off() && !d.hidden) {
            let (id, solid) = (d.layer.id.as_str(), d.layer.solid);
            match d.content {
                Content::Group { .. } => {
                    let inner =
                        (i + 1..d.end).filter(|j| !matches!(draws[*j].content, Content::Group { .. }));
                    for j in inner {
                        self.hitbox(j, (id, solid, Some(d.world)), (sources, covers), (&mut boxes.0, &mut n));
                    }
                }
                _ => self.hitbox(i, (id, solid, None), (sources, covers), (&mut boxes.0, &mut n)),
            }
        }
        boxes.0.truncate(n);
    }
}

impl SceneView {
    pub fn hitboxes(&self) -> &Hitboxes {
        &self.hitboxes
    }
}
