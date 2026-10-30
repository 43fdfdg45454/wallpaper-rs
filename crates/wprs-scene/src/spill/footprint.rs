use crate::spill::written::Written;
use crate::spill::{indexed, root};
use naga::proc::{Layouter, TypeResolution};
use naga::valid::ModuleInfo;
use naga::{Expression, Module, TypeInner};
use std::collections::HashSet;

pub(crate) fn footprint((module, info): (&Module, &ModuleInfo), written: &Written) -> Result<u64, String> {
    let mut layouter = Layouter::default();
    layouter.update(module.to_ctx()).map_err(|e| format!("its shader has a type without a size: {e}"))?;
    let size = |ty: &TypeResolution| match ty.inner_with(&module.types) {
        TypeInner::Pointer { base, .. } => u64::from(layouter[*base].size),
        _ => match ty {
            TypeResolution::Handle(h) => u64::from(layouter[*h].size),
            TypeResolution::Value(inner) => u64::from(inner.size(module.to_ctx())),
        },
    };
    let functions = module.functions.iter().map(|(h, f)| (f, &info[h]));
    let entries = module.entry_points.iter().enumerate().map(|(n, e)| (&e.function, info.get_entry_point(n)));
    let (mut seen, mut sum) = (HashSet::new(), 0u64);
    for (n, (f, fi)) in functions.chain(entries).enumerate() {
        for (h, e) in f.expressions.iter() {
            let Expression::Access { base, .. } = e else { continue };
            let at = root(f, *base);
            if !indexed(module, fi, f, h) || !written.at(f, at) {
                continue;
            }
            let key = match f.expressions[at] {
                Expression::GlobalVariable(g) => (usize::MAX, 0, g.index()),
                Expression::LocalVariable(l) => (n, 0, l.index()),
                _ => (n, 1, at.index()),
            };
            if seen.insert(key) {
                sum = sum.saturating_add(size(&fi[at].ty));
            }
        }
    }
    Ok(sum)
}
