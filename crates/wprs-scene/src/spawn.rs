use crate::scripted::{Extent, LayerRead};
use crate::{Kind, Layer, LayerOverride, MAX_LAYERS, MAX_SPAWNED, ParamKind, Runtime, ScriptOutputs};
use std::collections::HashMap;

const SCRIPTED: [ParamKind; 8] = [
    ParamKind::Float,
    ParamKind::Int,
    ParamKind::Enum,
    ParamKind::Vec2,
    ParamKind::Position,
    ParamKind::Color,
    ParamKind::Bool,
    ParamKind::Blend,
];

fn flat(layer: &Layer) -> usize {
    1 + layer.children().iter().map(flat).sum::<usize>()
}

fn renamed(layer: &mut Layer, id: &str) {
    layer.id = id.to_owned();
    if let Kind::Group { layers, .. } = &mut layer.kind {
        layers.iter_mut().for_each(|child| {
            let inner = format!("{id}.{}", child.id);
            renamed(child, &inner);
        });
    }
}

impl Runtime {
    pub(crate) fn adopt(&mut self, mut outputs: ScriptOutputs) -> bool {
        let mut kept: HashMap<String, (String, Box<Layer>)> =
            std::mem::take(&mut self.spawned).into_iter().map(|(t, l)| (l.id.clone(), (t, l))).collect();
        let mut room = (MAX_LAYERS + MAX_SPAWNED).saturating_sub(self.scene.layers().count());
        for (id, template) in &outputs.spawned {
            let Some(size) = self.scene.templates.get(template).map(flat).filter(|n| *n <= room) else {
                continue;
            };
            room -= size;
            let old = kept.remove(id).filter(|(t, _)| t == template);
            let made = old.or_else(|| {
                let mut layer = Box::new(self.scene.templates.get(template)?.clone());
                renamed(&mut layer, id);
                Some((template.clone(), layer))
            });
            self.spawned.extend(made);
        }
        outputs.spawned.retain(|(id, _)| self.spawned.iter().any(|(_, l)| l.id == *id));
        if outputs.spawned != self.script.spawned {
            self.costs.set((0, [None; 4]));
        }
        for (id, list) in outputs.effects.iter_mut() {
            let Some(layer) = self.layer(id) else {
                list.clear();
                continue;
            };
            list.retain(|c| c.at < layer.effects.len());
            for control in list.iter_mut() {
                let found = self.scene.shader(&layer.effects[control.at].effect);
                control.params.retain(|id, value| {
                    let def = found.and_then(|d| d.params.iter().find(|p| &p.id == id));
                    def.is_some_and(|p| SCRIPTED.contains(&p.kind) && p.accepts(value))
                });
            }
        }
        outputs.effects.retain(|_, list| !list.is_empty());
        let (wake, busy) = (outputs.wake, outputs.busy);
        (outputs.wake, outputs.busy, outputs.console) = (self.script.wake, self.script.busy, Vec::new());
        let changed = outputs != self.script;
        self.script = ScriptOutputs { wake, busy, ..outputs };
        changed
    }

    pub fn script_watch(&self) -> &[String] {
        &self.script.watch
    }

    pub fn fill_reads(&self, out: &mut Vec<(String, LayerRead)>) {
        let mut n = 0;
        for (id, read) in self.script.watch.iter().filter_map(|id| Some((id, self.read(id)?))) {
            match out.get_mut(n) {
                Some(slot) => {
                    if slot.0 != *id {
                        slot.0.clone_from(id);
                    }
                    slot.1 = read;
                }
                None => out.push((id.clone(), read)),
            }
            n += 1;
        }
        out.truncate(n);
    }

    pub fn script_busy(&self) -> bool {
        self.script.busy
    }

    pub fn spawned(&self) -> impl Iterator<Item = &Layer> {
        self.spawned.iter().map(|(_, l)| &**l)
    }

    pub fn layer(&self, id: &str) -> Option<&Layer> {
        self.scene.layer(id).or_else(|| {
            let mut stack: Vec<&Layer> = self.spawned().collect();
            while let Some(l) = stack.pop() {
                if l.id == id {
                    return Some(l);
                }
                stack.extend(l.children());
            }
            None
        })
    }

    pub(crate) fn arranged(&self) -> Option<Vec<&Layer>> {
        if self.spawned.is_empty() && self.script.order.is_none() {
            return None;
        }
        let mut top: Vec<&Layer> = self.scene.layers.iter().chain(self.spawned()).collect();
        if let Some(order) = &self.script.order {
            let rank = |l: &&Layer| order.iter().position(|id| *id == l.id).unwrap_or(usize::MAX);
            top.sort_by_key(rank);
        }
        Some(top)
    }

    pub fn read(&self, id: &str) -> Option<LayerRead> {
        let layer = self.layer(id)?;
        let cx = self.context();
        let none = LayerOverride::default();
        let set = self.script.layers.get(id).unwrap_or(&none);
        let t = &layer.transform;
        let size = match &layer.kind {
            Kind::Text(text) => Some(Extent::Points(text.size)),
            Kind::Image { size, .. } | Kind::Video { size, .. } | Kind::Solid { size, .. } => {
                size.map(Extent::Box)
            }
            Kind::Spectrum(s) => Some(Extent::Box(s.size)),
            _ => None,
        };
        let color = match &layer.kind {
            Kind::Image { color, .. } | Kind::Video { color, .. } | Kind::Group { color, .. } => {
                color.as_ref().map(|c| c.get(&cx).0)
            }
            Kind::Solid { color, .. } => Some(color.get(&cx).0),
            Kind::Text(text) => Some(text.color.get(&cx).0),
            Kind::Spectrum(s) => Some(s.color.get(&cx).0),
            Kind::Shape { fill, .. } => fill.as_ref().map(|c| c.get(&cx).0),
            _ => None,
        };
        let text = match &layer.kind {
            Kind::Text(text) => Some(set.text.clone().unwrap_or_else(|| text.text.get(&cx))),
            _ => None,
        };
        Some(LayerRead {
            position: set.position.or_else(|| t.position.as_ref().map(|p| p.get(&cx).0)).unwrap_or([0.0; 2]),
            scale: set.scale.unwrap_or_else(|| t.scale.get(&cx).0),
            rotation: set.rotation.unwrap_or_else(|| t.rotation.get(&cx)),
            skew: set.skew.unwrap_or_else(|| t.skew.get(&cx).0),
            origin: set.origin.or(t.origin).unwrap_or([0.5; 2]),
            opacity: set.opacity.unwrap_or_else(|| layer.opacity.get(&cx)),
            visible: set.visible.unwrap_or_else(|| layer.visible.get(&cx)),
            color: set.color.or(color).unwrap_or([1.0; 4]),
            parallax: set.parallax.unwrap_or(layer.parallax.0),
            size: set.size.or(size),
            text,
        })
    }
}
