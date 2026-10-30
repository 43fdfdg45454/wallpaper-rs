use crate::loops::{MAX_INLINED, MAX_OPS, MAX_SHADER_NODES};
use crate::spill::written::Written;
use crate::tally::Work;
use naga::valid::ModuleInfo;
use naga::{Function, Handle, Module, Override};
use std::collections::HashMap;

pub(crate) struct Walk<'m> {
    pub(crate) module: &'m Module,
    pub(crate) top: &'m dyn Fn(Handle<Override>) -> Option<f64>,
    pub(crate) info: &'m ModuleInfo,
    pub(crate) toll: u64,
    pub(crate) written: Written,
    pub(crate) done: HashMap<Handle<Function>, Work>,
}

pub(crate) fn work(
    (module, info): (&Module, &ModuleInfo),
    entries: &[&str],
    top: &dyn Fn(Handle<Override>) -> Option<f64>,
) -> Result<Vec<Work>, String> {
    let all = module.functions.iter().map(|(_, f)| f).chain(module.entry_points.iter().map(|e| &e.function));
    if all.map(|f| f.expressions.len()).sum::<usize>() > MAX_SHADER_NODES {
        return Err(format!("its shader has more than {MAX_SHADER_NODES} expressions"));
    }
    let written = Written::new(module);
    let toll = crate::spill::toll(crate::spill::footprint::footprint((module, info), &written)?);
    let mut walk = Walk { module, top, info, toll, written, done: HashMap::new() };
    let mut found = Vec::with_capacity(entries.len());
    for name in entries {
        let cost = match module.entry_points.iter().position(|e| e.name == *name) {
            Some(at) => {
                let e = &module.entry_points[at];
                walk.block((&e.function, info.get_entry_point(at)), &e.function.body)?
            }
            None => {
                let named = module.functions.iter().find(|(_, f)| f.name.as_deref() == Some(name));
                named.map_or(Ok(Work::default()), |(h, _)| walk.call(h))?
            }
        };
        if cost.ops > MAX_OPS {
            return Err(format!("{name} runs more than {MAX_OPS} operations a pixel"));
        }
        found.push(cost);
    }
    if let Some(n) = entries.iter().zip(&found).find(|(_, w)| w.size > MAX_INLINED).map(|(n, _)| n) {
        return Err(format!("{n} is more than {MAX_INLINED} operations long once its calls are counted"));
    }
    Ok(found)
}
