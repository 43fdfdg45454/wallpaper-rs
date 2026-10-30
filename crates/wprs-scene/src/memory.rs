use crate::shader::called;
use naga::proc::{Layouter, TypeResolution};
use naga::valid::{FunctionInfo, ModuleInfo};
use naga::{AddressSpace, Function, Handle, Module, TypeInner};
use std::collections::HashMap;

pub const MAX_MEMORY: u64 = 8 << 10;

pub(crate) fn memory((module, info): (&Module, &ModuleInfo)) -> Result<(), String> {
    let mut layouter = Layouter::default();
    layouter.update(module.to_ctx()).map_err(|e| format!("its shader has a type without a size: {e}"))?;
    let size = |ty| u64::from(layouter[ty].size);
    let shared = module
        .global_variables
        .iter()
        .filter(|(_, g)| matches!(g.space, AddressSpace::Private | AddressSpace::WorkGroup))
        .map(|(_, g)| size(g.ty))
        .fold(0, u64::saturating_add);
    let mut done = HashMap::new();
    let mut deepest = 0;
    for (at, e) in module.entry_points.iter().enumerate() {
        let held = chain(module, &size, &mut done, (&e.function, info.get_entry_point(at)), info);
        deepest = deepest.max(held);
    }
    if shared.saturating_add(deepest) > MAX_MEMORY {
        return Err(format!("its shader keeps more than {} KiB in variables a pixel", MAX_MEMORY >> 10));
    }
    Ok(())
}

fn chain(
    module: &Module,
    size: &dyn Fn(Handle<naga::Type>) -> u64,
    done: &mut HashMap<Handle<Function>, u64>,
    (f, fi): (&Function, &FunctionInfo),
    info: &ModuleInfo,
) -> u64 {
    let locals = f.local_variables.iter().map(|(_, v)| size(v.ty)).fold(0, u64::saturating_add);
    let kept = |ty: Handle<naga::Type>| match module.types[ty].inner {
        TypeInner::Array { .. } | TypeInner::Struct { .. } => size(ty),
        _ => 0,
    };
    let value = f.expressions.iter().map(|(h, _)| match &fi[h].ty {
        TypeResolution::Handle(ty) => kept(*ty),
        TypeResolution::Value(_) => 0,
    });
    let value = value.fold(0, u64::saturating_add);
    let mut calls = Vec::new();
    called(&f.body, &mut calls);
    let mut deepest = 0;
    for h in calls {
        let held = match done.get(&h) {
            Some(held) => *held,
            None => {
                let held = chain(module, size, done, (&module.functions[h], &info[h]), info);
                *done.entry(h).or_insert(held)
            }
        };
        deepest = deepest.max(held);
    }
    locals.saturating_add(value).saturating_add(deepest)
}
