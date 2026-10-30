use crate::custom::{CONTRACT, MAX_WGSL};
use crate::effects::Descriptor;
use crate::measure::measure;
use crate::memory::memory;
use crate::shader::{bindings, compile, reads};
use crate::value::Param;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

pub(crate) fn key(json: &str, wgsl: &str) -> u64 {
    let digest = Sha256::new().chain_update(json).chain_update([0]).chain_update(wgsl).finalize();
    u64::from_le_bytes(digest[..8].try_into().unwrap_or_default()) | 1
}

pub(crate) fn checked(id: &str, json: &str, wgsl: &str) -> Result<Descriptor, String> {
    let raw: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut d = Descriptor::parse(json, wgsl).map_err(|e| e.to_string())?;
    let taken = crate::reserved(&d.id);
    let checks = [
        (!taken, format!("{} is the name of a built-in effect", d.id)),
        (d.id == id, format!("declares the id {}, expected {id}", d.id)),
        (raw.get("cost").is_some(), "needs a cost".into()),
        (wgsl.len() <= MAX_WGSL, format!("its shader is larger than {} KiB", MAX_WGSL >> 10)),
    ];
    if let Some((_, problem)) = checks.into_iter().find(|(ok, _)| !ok) {
        return Err(problem);
    }
    if let Some(problem) = d.problems().into_iter().next() {
        return Err(problem);
    }
    let source = crate::guard::source(&d);
    let (module, info) = compile(&source)?;
    d.key = key(json, wgsl);
    let entries: Vec<&str> =
        d.passes.iter().flat_map(|p| [p.entry.as_str()].into_iter().chain(p.vertex.as_deref())).collect();
    let mut entries: Vec<&str> = entries.into_iter().filter(|e| !e.is_empty()).collect();
    if let Some(missing) = entries.iter().find(|e| !module.entry_points.iter().any(|p| p.name == **e)) {
        return Err(format!("its shader has no entry point {missing}"));
    }
    if let Some((name, kind)) = d.stage.function() {
        let typed = |h: naga::Handle<naga::Type>| module.types[h].name.as_deref() == Some(kind);
        let found = module.functions.iter().any(|(_, f)| {
            let args = matches!(f.arguments.as_slice(), [a] if typed(a.ty));
            f.name.as_deref() == Some(name) && args && f.result.as_ref().is_some_and(|r| typed(r.ty))
        });
        if !found {
            return Err(format!("its shader needs fn {name}(value: {kind}) -> {kind}"));
        }
        entries.push(name);
    }
    if let Some(o) =
        d.options.iter().find(|o| !module.overrides.iter().any(|(_, v)| v.name.as_deref() == Some(&o.id)))
    {
        return Err(format!("its shader declares no override {}", o.id));
    }
    let range = |o: &crate::Variant| {
        let found = module.overrides.iter().find(|(_, v)| v.name.as_deref() == Some(&o.id));
        let kind = found.and_then(|(_, v)| module.types[v.ty].inner.scalar_kind());
        let (lo, hi) = match kind {
            Some(naga::ScalarKind::Sint) => (f64::from(i32::MIN), f64::from(i32::MAX)),
            Some(naga::ScalarKind::Uint | naga::ScalarKind::Bool) => (0.0, f64::from(u32::MAX)),
            _ => (f64::MIN, f64::MAX),
        };
        o.values.iter().filter_map(Param::whole).all(|v| (lo..=hi).contains(&v))
    };
    if let Some(o) = d.options.iter().find(|o| !range(o)) {
        return Err(format!("the option {} has values its override cannot hold", o.id));
    }
    memory((&module, &info))?;
    let (sampled, ops, size) = measure(&d, (&module, &info), &entries)?;
    (d.reads, d.ops, d.size) = (sampled, ops, size);
    static ALLOWED: OnceLock<usize> = OnceLock::new();
    let allowed = *ALLOWED.get_or_init(|| compile(CONTRACT).map_or(0, |(m, _)| bindings(&m)));
    if bindings(&module) > allowed {
        return Err("its shader declares bindings of its own".into());
    }
    for found in reads(&module, &entries) {
        if !d.uses.contains(&found) {
            d.uses.push(found);
        }
    }
    crate::guard::armed(module, &source)?;
    Ok(d)
}

impl Descriptor {
    pub fn packaged(id: &str, json: &str, wgsl: &str) -> Result<Descriptor, String> {
        let trial = crate::package::key(json, wgsl) & !1;
        let again = |e| format!("its shader is not checked again: {e}");
        if crate::struck(trial) {
            return Err(again(crate::STOPPED.into()));
        }
        crate::verdicts::vetted(id, json, wgsl)?;
        crate::trying(trial).map_err(again)?;
        let checked = crate::shader::roomy(|| crate::package::checked(id, json, wgsl));
        crate::tried(trial);
        checked.inspect(|d| crate::verdicts::pass(d.key))
    }
}
