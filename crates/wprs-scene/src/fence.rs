use crate::loops::{MAX_LOOP, MAX_WORK};
use naga::{AddressSpace, BinaryOperator, Block, Expression, Function, GlobalVariable, Handle, Literal};
use naga::{LocalVariable, MemoryDecorations, Module, Scalar, Span, Statement, Type, TypeInner};

struct Guard {
    ty: Handle<Type>,
    total: Handle<GlobalVariable>,
}

pub(crate) fn guard(module: &mut Module) {
    let ty = module.types.insert(Type { name: None, inner: TypeInner::Scalar(Scalar::U32) }, Span::UNDEFINED);
    let total = GlobalVariable {
        name: None,
        space: AddressSpace::Private,
        binding: None,
        ty,
        init: None,
        memory_decorations: MemoryDecorations::empty(),
    };
    let g = Guard { ty, total: module.global_variables.append(total, Span::UNDEFINED) };
    for (_, f) in module.functions.iter_mut() {
        crate::hoist::hoist(f);
        g.function(f, false);
    }
    for e in module.entry_points.iter_mut() {
        crate::hoist::hoist(&mut e.function);
        g.function(&mut e.function, true);
    }
}

impl Guard {
    fn function(&self, f: &mut Function, entry: bool) {
        let mut body = std::mem::take(&mut f.body);
        self.block(f, &mut body);
        if entry {
            let pointer = f.expressions.append(Expression::GlobalVariable(self.total), Span::UNDEFINED);
            let value = f.expressions.append(Expression::Literal(Literal::U32(0)), Span::UNDEFINED);
            let mut reset = Block::new();
            reset.push(Statement::Store { pointer, value }, Span::UNDEFINED);
            reset.extend_block(body);
            body = reset;
        }
        f.body = body;
    }

    fn block(&self, f: &mut Function, b: &mut Block) {
        for (mut s, span) in std::mem::take(b).span_into_iter() {
            match &mut s {
                Statement::Block(inner) => self.block(f, inner),
                Statement::If { accept, reject, .. } => {
                    self.block(f, accept);
                    self.block(f, reject);
                }
                Statement::Switch { cases, .. } => cases.iter_mut().for_each(|c| self.block(f, &mut c.body)),
                Statement::Loop { body, continuing, .. } => {
                    self.block(f, body);
                    self.block(f, continuing);
                    let local = LocalVariable { name: None, ty: self.ty, init: None };
                    let local = f.local_variables.append(local, span);
                    let counter = f.expressions.append(Expression::LocalVariable(local), span);
                    let zero = f.expressions.append(Expression::Literal(Literal::U32(0)), span);
                    b.push(Statement::Store { pointer: counter, value: zero }, span);
                    let total = f.expressions.append(Expression::GlobalVariable(self.total), span);
                    let mut top = Block::new();
                    tick(f, &mut top, counter, MAX_LOOP, span);
                    tick(f, &mut top, total, MAX_WORK, span);
                    top.extend_block(std::mem::take(body));
                    *body = top;
                }
                _ => {}
            }
            b.push(s, span);
        }
    }
}

fn tick(f: &mut Function, out: &mut Block, pointer: Handle<Expression>, most: u64, span: Span) {
    let one = f.expressions.append(Expression::Literal(Literal::U32(1)), span);
    let most = u32::try_from(most).unwrap_or(u32::MAX);
    let most = f.expressions.append(Expression::Literal(Literal::U32(most)), span);
    let start = f.expressions.len();
    let now = f.expressions.append(Expression::Load { pointer }, span);
    let value =
        f.expressions.append(Expression::Binary { op: BinaryOperator::Add, left: now, right: one }, span);
    let over = f
        .expressions
        .append(Expression::Binary { op: BinaryOperator::Greater, left: value, right: most }, span);
    out.push(Statement::Emit(f.expressions.range_from(start)), span);
    out.push(Statement::Store { pointer, value }, span);
    let stop = Block::from_vec(vec![Statement::Break]);
    out.push(Statement::If { condition: over, accept: stop, reject: Block::new() }, span);
}
