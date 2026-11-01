use crate::{Layer, MAX_DEPTH, Runtime, ScriptOutputs};

impl Runtime {
    pub(crate) fn parents(&self, outputs: &mut ScriptOutputs) {
        let wanted: Vec<(String, String)> = (outputs.layers.iter())
            .filter_map(|(id, set)| {
                set.parent.as_ref().filter(|p| !p.is_empty()).map(|p| (id.clone(), p.clone()))
            })
            .collect();
        for (id, parent) in wanted {
            let parent_of = |at: &str| match outputs.layers.get(at).and_then(|s| s.parent.as_deref()) {
                Some("") => None,
                Some(p) => Some(p.to_owned()),
                None => self.scene.layer(at).and_then(|l| l.parent.clone()),
            };
            let (mut at, mut depth, mut fine) = (Some(parent.clone()), 0, true);
            while let Some(step) = at {
                fine &= step != id && depth < MAX_DEPTH && self.scene.layer(&step).is_some_and(Layer::drawn);
                (at, depth) = (if fine { parent_of(&step) } else { None }, depth + 1);
            }
            if !fine {
                outputs.layers.get_mut(&id).iter_mut().for_each(|set| set.parent = None);
            }
        }
    }

    pub(crate) fn molds(&self, outputs: &mut ScriptOutputs) {
        outputs.meshes.retain(|id, mesh| self.scene.pools.get(id).is_some_and(|pool| mesh.fits(pool)));
    }

    pub fn script(&self) -> &ScriptOutputs {
        &self.script
    }

    pub fn mesh(&self, pool: &str) -> Option<&crate::Mesh> {
        self.script.meshes.get(pool).map(|m| &**m)
    }
}
