mod expr;
mod places;
mod value;
mod walk;

use crate::effects::Descriptor;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};
use value::V;
use walk::Walk;

const KEPT: usize = 512;

type Parsed = Option<Arc<naga::Module>>;

fn parsed(d: &Descriptor) -> Parsed {
    static SEEN: OnceLock<Mutex<HashMap<(u64, String), Parsed>>> = OnceLock::new();
    let seen = SEEN.get_or_init(Default::default);
    let key = (d.key, d.id.clone());
    if let Some(found) = seen.lock().ok().and_then(|s| s.get(&key).cloned()) {
        return found;
    }
    let found = crate::local::module_of(d).map(Arc::new);
    if let Ok(mut s) = seen.lock() {
        s.insert(key, found.clone());
    }
    found
}

impl Descriptor {
    pub fn spread(&self, fx: &[[f32; 4]], units: bool) -> Option<[f32; 2]> {
        static SEEN: OnceLock<Mutex<HashMap<u64, Option<[f32; 2]>>>> = OnceLock::new();
        let mut h = std::hash::DefaultHasher::new();
        (self.key, &self.id, units, fx.iter().flatten().map(|v| v.to_bits()).collect::<Vec<_>>())
            .hash(&mut h);
        let (seen, key) = (SEEN.get_or_init(Default::default), h.finish());
        if let Some(found) = seen.lock().ok().and_then(|s| s.get(&key).copied()) {
            return found;
        }
        let found = self.measure(fx, units);
        if let Ok(mut s) = seen.lock() {
            if s.len() >= KEPT {
                s.clear();
            }
            s.insert(key, found);
        }
        found
    }

    fn measure(&self, fx: &[[f32; 4]], units: bool) -> Option<[f32; 2]> {
        let [pass] = self.passes.as_slice() else { return None };
        let own = |i: &String| matches!(i.as_str(), "previous" | "input");
        if pass.vertex.is_some() || pass.grid.is_some() || pass.inputs.iter().skip(1).any(own) {
            return None;
        }
        let m = parsed(self)?;
        let f = &m.entry_points.iter().find(|e| e.name == pass.entry)?.function;
        let uv = f.arguments.first().and_then(|a| match &m.types[a.ty].inner {
            naga::TypeInner::Struct { members, .. } => {
                members.iter().position(|s| s.name.as_deref() == Some("uv")).map(|i| i as u32)
            }
            _ => None,
        });
        let mut walk = Walk::new(&m, fx, units);
        let mut frame = walk.frame(f, vec![V::Any], uv)?;
        walk.run(&mut frame);
        let most = walk.reads.iter().fold([0.0f64; 2], |a, d| [a[0].max(d[0].most()), a[1].max(d[1].most())]);
        (!walk.bad && most.iter().all(|v| v.is_finite())).then(|| most.map(|v| v as f32))
    }
}
