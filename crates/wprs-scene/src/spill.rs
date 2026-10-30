use crate::spill::written::Written;
use naga::valid::FunctionInfo;
use naga::{AddressSpace, Expression, Function, Handle, Module, TypeInner};

pub(crate) mod footprint;
pub(crate) mod written;

pub(crate) const SPILL_STEP: u64 = 1 << 10;

pub(crate) fn toll(bytes: u64) -> u64 {
    bytes.saturating_mul(bytes.min(SPILL_STEP)) / 32
}

fn spills(module: &Module, fi: &FunctionInfo, h: Handle<Expression>) -> bool {
    let inner = match fi[h].ty.inner_with(&module.types) {
        TypeInner::Pointer { base, space: AddressSpace::Function | AddressSpace::Private } => {
            &module.types[*base].inner
        }
        TypeInner::Pointer { .. } | TypeInner::ValuePointer { .. } => return false,
        inner => inner,
    };
    matches!(inner, TypeInner::Array { .. } | TypeInner::Matrix { .. })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reach {
    Plain,
    Fetch,
    Spill,
}

pub(crate) fn indexed(module: &Module, fi: &FunctionInfo, f: &Function, h: Handle<Expression>) -> bool {
    let Expression::Access { base, index } = f.expressions[h] else { return false };
    let fixed = matches!(
        f.expressions[index],
        Expression::Literal(_) | Expression::Constant(_) | Expression::Override(_) | Expression::ZeroValue(_)
    );
    !fixed && spills(module, fi, base)
}

pub(crate) fn through(
    at: (&Module, &FunctionInfo, &Written),
    f: &Function,
    mut p: Handle<Expression>,
) -> Reach {
    let (module, fi, written) = at;
    let mut hit = false;
    loop {
        hit |= indexed(module, fi, f, p);
        match f.expressions[p] {
            Expression::Access { base, .. } | Expression::AccessIndex { base, .. } => p = base,
            _ if !hit => return Reach::Plain,
            _ if written.at(f, p) => return Reach::Spill,
            _ => return Reach::Fetch,
        }
    }
}

pub(crate) fn root(f: &Function, mut p: Handle<Expression>) -> Handle<Expression> {
    while let Expression::Access { base, .. } | Expression::AccessIndex { base, .. } = f.expressions[p] {
        p = base;
    }
    p
}

pub(crate) fn touches(at: (&Module, &FunctionInfo, &Written), f: &Function, h: Handle<Expression>) -> Reach {
    let pointer = matches!(
        at.1[h].ty.inner_with(&at.0.types),
        TypeInner::Pointer { .. } | TypeInner::ValuePointer { .. }
    );
    match f.expressions[h] {
        Expression::Load { pointer } => through(at, f, pointer),
        Expression::Access { .. } if !pointer => through(at, f, h),
        _ => Reach::Plain,
    }
}
