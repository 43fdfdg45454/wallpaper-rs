use crate::validate::{Check, Issue, nested};
use crate::value::{Binding, Param};
use crate::{Layer, MAX_DEPTH, ParamDef, ParamKind};
use std::collections::HashMap;

impl Check<'_> {
    pub(crate) fn fed(&mut self, path: &str, def: &ParamDef, b: &Binding<Param>) {
        self.uses(true, "layer_textures");
        let id = b.layer.as_deref().unwrap_or_default();
        let drawn = self.scene.layer(id).is_some_and(Layer::drawn);
        self.known(&format!("{path}/layer"), "layer", id, drawn);
        if !matches!(def.kind, ParamKind::Texture | ParamKind::Mask) {
            self.issues.push(Issue::Type(path.into()));
        }
    }

    pub(crate) fn parents(&mut self) {
        let scene = self.scene;
        let mut paths = Vec::new();
        nested("/layers", &scene.layers, &mut paths);
        let up: HashMap<&str, &str> =
            scene.layers().filter_map(|l| Some((l.id.as_str(), l.parent.as_deref()?))).collect();
        for (path, id) in &paths {
            let Some(parent) = up.get(id) else { continue };
            let usable = *parent != *id && scene.layer(parent).is_some_and(Layer::drawn);
            self.known(&format!("{path}/parent"), "layer", parent, usable);
            let mut chain = std::iter::successors(Some(*id), |at| up.get(at).copied()).skip(1);
            match chain.by_ref().take(MAX_DEPTH).position(|at| at == *id) {
                Some(_) => self.issues.push(Issue::Circle(format!("{path}/parent"))),
                None if chain.next().is_some() => self.issues.push(Issue::Depth(format!("{path}/parent"))),
                None => {}
            }
        }
        let mut edges: HashMap<&str, Vec<&str>> = HashMap::new();
        for l in scene.layers() {
            self.uses(fed(l).next().is_some(), "layer_textures");
            let fed: Vec<&str> = fed(l).chain(matted(l)).collect();
            let to = edges.entry(l.id.as_str()).or_default();
            to.extend(l.mask.as_ref().and_then(|m| m.layer.as_deref()));
            to.extend(l.children().iter().map(|c| c.id.as_str()));
            to.extend(fed);
        }
        for (path, id) in &paths {
            let mut stack: Vec<&str> =
                scene.layer(id).into_iter().flat_map(|l| fed(l).chain(matted(l))).collect();
            let mut seen = std::collections::HashSet::new();
            while let Some(at) = stack.pop() {
                if at == *id {
                    self.issues.push(Issue::Circle(format!("{path}/effects")));
                    break;
                }
                if seen.insert(at) {
                    stack.extend(edges.get(at).into_iter().flatten());
                }
            }
        }
    }
}

pub(crate) fn reads(l: &Layer) -> impl Iterator<Item = &str> {
    fed(l).chain(matted(l)).chain(l.mask.as_ref().and_then(|m| m.layer.as_deref()))
}

fn fed(l: &Layer) -> impl Iterator<Item = &str> {
    l.effects.iter().flat_map(|e| e.params.values()).filter_map(|v| v.binding()?.layer.as_deref())
}

fn matted(l: &Layer) -> impl Iterator<Item = &str> {
    l.effects.iter().filter_map(|e| e.mask.as_ref()?.layer.as_deref())
}
