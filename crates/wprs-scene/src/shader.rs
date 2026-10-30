use crate::Use;
use naga::valid::{Capabilities, ModuleInfo, ValidationFlags, Validator};
use naga::{Block, Expression, Function, Handle, Module, Statement, TypeInner};
use std::collections::HashSet;

mod tree;

const ROOM: usize = 64 << 20;

const READS: [(&str, Use); 19] = [
    ("time", Use::Time),
    ("dt", Use::Time),
    ("daytime", Use::Time),
    ("cursor", Use::Cursor),
    ("cursor_delta", Use::Cursor),
    ("cursor_last", Use::Cursor),
    ("inside", Use::Cursor),
    ("presence", Use::Cursor),
    ("parallax", Use::Cursor),
    ("buttons", Use::Clicks),
    ("bass", Use::Audio),
    ("mid", Use::Audio),
    ("treble", Use::Audio),
    ("level", Use::Audio),
    ("beat", Use::Audio),
    ("since_beat", Use::Audio),
    ("audio", Use::Audio),
    ("bands_l", Use::Audio),
    ("bands_r", Use::Audio),
];

pub(crate) fn compile(source: &str) -> Result<(Module, ModuleInfo), String> {
    crate::shape::shape(source)?;
    let module = naga::front::wgsl::parse_str(source).map_err(|e| e.emit_to_string(source))?;
    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::default());
    let info = validator.validate(&module).map_err(|e| e.emit_to_string(source))?;
    crate::shader::tree::tree(&module)?;
    Ok((module, info))
}

pub(crate) fn roomy<T: Send>(work: impl FnOnce() -> Result<T, String> + Send) -> Result<T, String> {
    std::thread::scope(|s| {
        let thread = std::thread::Builder::new().name("wprs-shader".into()).stack_size(ROOM);
        let worker = thread.spawn_scoped(s, work).map_err(|e| format!("cannot check its shader: {e}"))?;
        worker.join().unwrap_or_else(|_| Err("its shader could not be checked".into()))
    })
}

pub(crate) fn bindings(module: &Module) -> usize {
    module.global_variables.iter().filter(|(_, g)| g.binding.is_some()).count()
}

pub(crate) fn reads(module: &Module, entries: &[&str]) -> Vec<Use> {
    let members = |e: &Expression, f: &Function| -> Vec<&str> {
        let (global, index) = match e {
            Expression::AccessIndex { base, index } => (&f.expressions[*base], Some(*index as usize)),
            Expression::Load { pointer } => (&f.expressions[*pointer], None),
            _ => return Vec::new(),
        };
        let Expression::GlobalVariable(g) = global else { return Vec::new() };
        let TypeInner::Struct { members, .. } = &module.types[module.global_variables[*g].ty].inner else {
            return Vec::new();
        };
        let picked = members.iter().enumerate().filter(|(i, _)| index.is_none_or(|n| n == *i));
        picked.filter_map(|(_, m)| m.name.as_deref()).collect()
    };
    let named = module
        .functions
        .iter()
        .map(|(_, f)| f)
        .filter(|f| f.name.as_deref().is_some_and(|n| entries.contains(&n)));
    let mut stack: Vec<&Function> = module
        .entry_points
        .iter()
        .filter(|e| entries.contains(&e.name.as_str()))
        .map(|e| &e.function)
        .chain(named)
        .collect();
    let (mut seen, mut found) = (HashSet::new(), Vec::new());
    while let Some(f) = stack.pop() {
        for (_, e) in f.expressions.iter() {
            for name in members(e, f) {
                if let Some((_, used)) = READS.iter().find(|(m, _)| *m == name)
                    && !found.contains(used)
                {
                    found.push(*used);
                }
            }
        }
        let mut calls = Vec::new();
        called(&f.body, &mut calls);
        stack.extend(calls.into_iter().filter(|c| seen.insert(*c)).map(|c| &module.functions[c]));
    }
    found
}

pub(crate) fn called(block: &Block, out: &mut Vec<Handle<Function>>) {
    for statement in block.iter() {
        match statement {
            Statement::Call { function, .. } => out.push(*function),
            Statement::Block(b) => called(b, out),
            Statement::If { accept, reject, .. } => [accept, reject].into_iter().for_each(|b| called(b, out)),
            Statement::Switch { cases, .. } => cases.iter().for_each(|c| called(&c.body, out)),
            Statement::Loop { body, continuing, .. } => {
                [body, continuing].into_iter().for_each(|b| called(b, out))
            }
            _ => {}
        }
    }
}
