use crate::{Kind, Layer, Reach, Scene};
use std::collections::HashMap;

pub(crate) fn flat(layer: &Layer) -> usize {
    1 + layer.children().iter().map(flat).sum::<usize>()
}

fn names(layer: &Layer, id: &str, out: &mut HashMap<String, String>) {
    out.entry(layer.id.clone()).or_insert_with(|| id.to_owned());
    layer.children().iter().for_each(|child| names(child, &format!("{id}.{}", child.id), out));
}

fn renamed(layer: &mut Layer, id: &str, to: &HashMap<String, String>) {
    layer.id = id.to_owned();
    let swap = |name: &mut String| {
        if let Some(found) = to.get(name.as_str()) {
            name.clone_from(found);
        }
    };
    layer.attach.iter_mut().for_each(|a| swap(&mut a.layer));
    layer.parent.iter_mut().for_each(swap);
    layer.mask.iter_mut().filter_map(|m| m.layer.as_mut()).for_each(swap);
    for e in &mut layer.effects {
        e.mask.iter_mut().filter_map(|m| m.layer.as_mut()).for_each(swap);
        let fed = e.params.values_mut().filter_map(|v| match v {
            crate::Value::Bound(b) => b.layer.as_mut(),
            crate::Value::Fixed(_) => None,
        });
        fed.for_each(swap);
    }
    match &mut layer.kind {
        Kind::Group { layers, .. } => layers.iter_mut().for_each(|child| {
            let inner = format!("{id}.{}", child.id);
            renamed(child, &inner, to);
        }),
        Kind::Mesh(m) => {
            for ik in &mut m.rig.ik {
                let Reach::Named(target) = &mut ik.target else { continue };
                if let Some(found) = target.strip_prefix("layer:").and_then(|t| to.get(t)) {
                    *target = format!("layer:{found}");
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn copied(template: &Layer, id: &str) -> Layer {
    let mut to = HashMap::new();
    names(template, id, &mut to);
    let mut layer = template.clone();
    renamed(&mut layer, id, &to);
    layer
}

pub(crate) fn found<'a>(
    scene: &'a Scene,
    spawned: &'a [(String, Box<Layer>)],
    id: &str,
) -> Option<&'a Layer> {
    scene.layer(id).or_else(|| {
        let mut stack: Vec<&Layer> = spawned.iter().map(|(_, l)| &**l).collect();
        while let Some(l) = stack.pop() {
            if l.id == id {
                return Some(l);
            }
            stack.extend(l.children());
        }
        None
    })
}

impl<'a> crate::resolve::Context<'a> {
    pub(crate) fn find(&self, id: &str) -> Option<&'a Layer> {
        found(self.scene, self.spawned, id)
    }
}
