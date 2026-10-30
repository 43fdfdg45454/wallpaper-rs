use crate::effects::{Descriptor, PassDef};
use crate::loops::work;
use crate::tally::Work;
use crate::value::Param;
use naga::Module;
use naga::valid::ModuleInfo;

pub const MAX_BUILD: u64 = 1 << 17;
const OVERDRAW: f32 = 9.0;

pub(crate) fn spread(p: &PassDef) -> f32 {
    p.grid.map_or(1.0, |[x, y]| OVERDRAW.min(2.0 * x as f32 * y as f32))
}

pub(crate) fn measure(
    d: &Descriptor,
    (module, info): (&Module, &ModuleInfo),
    entries: &[&str],
) -> Result<(f32, f32, u64), String> {
    let named = |o: naga::Handle<naga::Override>| module.overrides[o].name.clone();
    let top = |o| {
        let option = d.options.iter().find(|v| Some(&v.id) == named(o).as_ref())?;
        option.values.iter().filter_map(Param::whole).reduce(f64::max)
    };
    let found = work((module, info), entries, &top)?;
    let of = |name: &str| entries.iter().position(|e| *e == name).map_or(Work::default(), |at| found[at]);
    let side = |p: &PassDef| p.target.as_ref().and_then(|t| d.targets.get(t)).map_or(p.scale, |t| t.scale);
    let drawn = d.passes.iter().filter(|p| !p.entry.is_empty());
    let area = |p: &PassDef, w: u64| w as f32 * side(p).powi(2) * spread(p);
    let reads = drawn.clone().map(|p| area(p, of(&p.entry).reads)).sum();
    let stage = d.stage.function().map_or(Work::default(), |(name, _)| of(name));
    let ops = drawn.clone().map(|p| area(p, of(&p.entry).ops)).sum::<f32>() + stage.ops as f32;
    let built = |p: &PassDef| of(&p.entry).size.saturating_add(p.vertex.as_deref().map_or(0, |v| of(v).size));
    let size = drawn.map(built).fold(stage.size, u64::saturating_add);
    if size > MAX_BUILD {
        return Err(format!("its passes are more than {MAX_BUILD} operations long together"));
    }
    Ok((reads, ops, size))
}
