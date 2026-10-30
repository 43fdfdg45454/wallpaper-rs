use crate::spill::root;
use naga::{
    Block, Expression, Function, GlobalVariable, Handle, LocalVariable, Module, Statement, TypeInner,
};
use std::collections::HashSet;

#[derive(Default)]
pub(crate) struct Written {
    locals: HashSet<(usize, Handle<LocalVariable>)>,
    globals: HashSet<Handle<GlobalVariable>>,
    pointers: HashSet<(usize, u32)>,
}

fn key(f: &Function) -> usize {
    std::ptr::from_ref(f).addr()
}

impl Written {
    pub(crate) fn new(module: &Module) -> Written {
        let mut w = Written::default();
        let all =
            module.functions.iter().map(|(_, f)| f).chain(module.entry_points.iter().map(|e| &e.function));
        for f in all {
            w.block(f, &f.body);
            for (i, a) in f.arguments.iter().enumerate() {
                if matches!(module.types[a.ty].inner, TypeInner::Pointer { .. }) {
                    w.pointers.insert((key(f), i as u32));
                }
            }
        }
        w
    }

    fn mark(&mut self, f: &Function, p: Handle<Expression>) {
        match f.expressions[root(f, p)] {
            Expression::LocalVariable(l) => drop(self.locals.insert((key(f), l))),
            Expression::GlobalVariable(g) => drop(self.globals.insert(g)),
            _ => {}
        }
    }

    fn block(&mut self, f: &Function, b: &Block) {
        for s in b.iter() {
            match s {
                Statement::Store { pointer, .. } if root(f, *pointer) != *pointer => self.mark(f, *pointer),
                Statement::Call { arguments, .. } => arguments.iter().for_each(|a| self.mark(f, *a)),
                Statement::Block(b) => self.block(f, b),
                Statement::If { accept, reject, .. } => {
                    [accept, reject].into_iter().for_each(|b| self.block(f, b))
                }
                Statement::Switch { cases, .. } => cases.iter().for_each(|c| self.block(f, &c.body)),
                Statement::Loop { body, continuing, .. } => {
                    [body, continuing].into_iter().for_each(|b| self.block(f, b))
                }
                _ => {}
            }
        }
    }

    pub(crate) fn at(&self, f: &Function, root: Handle<Expression>) -> bool {
        match f.expressions[root] {
            Expression::LocalVariable(l) => self.locals.contains(&(key(f), l)),
            Expression::GlobalVariable(g) => self.globals.contains(&g),
            Expression::FunctionArgument(i) => self.pointers.contains(&(key(f), i)),
            _ => false,
        }
    }
}
