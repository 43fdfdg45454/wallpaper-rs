use crate::chain::effect_source;
use crate::effects::Descriptor;
use naga::{Block, Expression, Function, Handle, Module, Statement, TypeInner};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const AT_PIXEL: [&str; 2] = ["tap", "mask_at"];
const PIXEL_ONLY: [&str; 2] = ["input_tex", "mask_tex"];
const DEPTH: u32 = 32;

impl Descriptor {
    pub fn reads_its_pixel(&self) -> bool {
        static SEEN: OnceLock<Mutex<HashMap<u64, HashMap<String, bool>>>> = OnceLock::new();
        let seen = SEEN.get_or_init(Default::default);
        let known =
            |s: &HashMap<u64, HashMap<String, bool>>| s.get(&self.key)?.get(self.id.as_str()).copied();
        if let Some(found) = seen.lock().ok().and_then(|s| known(&s)) {
            return found;
        }
        let found = judge(self);
        if let Ok(mut s) = seen.lock() {
            s.entry(self.key).or_default().insert(self.id.clone(), found);
        }
        found
    }
}

fn judge(d: &Descriptor) -> bool {
    let [pass] = d.passes.as_slice() else { return false };
    let own = |i: &String| matches!(i.as_str(), "previous" | "input");
    if pass.vertex.is_some() || pass.grid.is_some() || pass.inputs.iter().skip(1).any(own) {
        return false;
    }
    let Ok(module) = naga::front::wgsl::parse_str(&effect_source(&[d])) else { return false };
    let Some(entry) = module.entry_points.iter().find(|e| e.name == pass.entry) else { return false };
    let f = &entry.function;
    let uv = f.arguments.first().and_then(|a| match &module.types[a.ty].inner {
        TypeInner::Struct { members, .. } => members.iter().position(|m| m.name.as_deref() == Some("uv")),
        _ => None,
    });
    clean(&module, f, (uv.map(|i| i as u32), &[]), 0)
}

fn clean(m: &Module, f: &Function, (uv, pixel): (Option<u32>, &[u32]), depth: u32) -> bool {
    let fixed = |g: Handle<naga::GlobalVariable>| {
        m.global_variables[g].name.as_deref().is_some_and(|n| PIXEL_ONLY.contains(&n))
    };
    let touches = f.expressions.iter().any(|(_, e)| matches!(e, Expression::GlobalVariable(g) if fixed(*g)));
    if depth > DEPTH || touches {
        return false;
    }
    let at_pixel = |h: Handle<Expression>| match (uv, &f.expressions[h]) {
        (Some(i), Expression::AccessIndex { base, index }) => {
            *index == i && matches!(f.expressions[*base], Expression::FunctionArgument(0))
        }
        (_, Expression::FunctionArgument(k)) => pixel.contains(k),
        _ => false,
    };
    let mut found = Vec::new();
    calls(&f.body, &mut found);
    found.into_iter().all(|(h, args)| match m.functions[h].name.as_deref() {
        Some(n) if AT_PIXEL.contains(&n) => args.first().is_some_and(|a| at_pixel(*a)),
        _ => {
            let passed: Vec<u32> = (0..args.len() as u32).filter(|k| at_pixel(args[*k as usize])).collect();
            clean(m, &m.functions[h], (None, &passed), depth + 1)
        }
    })
}

fn calls(b: &Block, out: &mut Vec<(Handle<Function>, Vec<Handle<Expression>>)>) {
    for s in b.iter() {
        match s {
            Statement::Call { function, arguments, .. } => out.push((*function, arguments.clone())),
            Statement::Block(b) => calls(b, out),
            Statement::If { accept, reject, .. } => {
                calls(accept, out);
                calls(reject, out);
            }
            Statement::Loop { body, continuing, .. } => {
                calls(body, out);
                calls(continuing, out);
            }
            Statement::Switch { cases, .. } => cases.iter().for_each(|c| calls(&c.body, out)),
            _ => {}
        }
    }
}
