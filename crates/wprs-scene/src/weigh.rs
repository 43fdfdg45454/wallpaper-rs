use naga::valid::FunctionInfo;
use naga::{
    ArraySize, BinaryOperator, Expression, Function, Handle, MathFunction as M, Module, ScalarKind, TypeInner,
};

pub const SAMPLE: u64 = 16;
pub const DIVIDE: u64 = 32;

pub(crate) fn scalars(module: &Module, inner: &TypeInner) -> u64 {
    match inner {
        TypeInner::Scalar(_) | TypeInner::Atomic(_) => 1,
        TypeInner::Vector { size, .. } => *size as u64,
        TypeInner::Matrix { columns, rows, .. } => *columns as u64 * *rows as u64,
        TypeInner::Array { base, size: ArraySize::Constant(n), .. } => {
            u64::from(n.get()).saturating_mul(scalars(module, &module.types[*base].inner))
        }
        TypeInner::Struct { members, .. } => {
            members.iter().map(|m| scalars(module, &module.types[m.ty].inner)).fold(0, u64::saturating_add)
        }
        _ => 0,
    }
}

pub(crate) fn sized(module: &Module, info: &FunctionInfo, h: Handle<Expression>) -> u64 {
    scalars(module, info[h].ty.inner_with(&module.types)).max(1)
}

pub(crate) fn weight(module: &Module, info: &FunctionInfo, f: &Function, h: Handle<Expression>) -> u64 {
    let inner = |h: Handle<Expression>| info[h].ty.inner_with(&module.types);
    let n = |h| sized(module, info, h);
    match &f.expressions[h] {
        Expression::ImageSample { .. } | Expression::ImageLoad { .. } => SAMPLE,
        Expression::Binary { op: BinaryOperator::Multiply, left, right } => {
            match (inner(*left), inner(*right)) {
                (TypeInner::Matrix { columns, rows, .. }, TypeInner::Matrix { columns: wide, .. }) => {
                    *rows as u64 * *wide as u64 * *columns as u64
                }
                (TypeInner::Matrix { columns, rows, .. }, TypeInner::Vector { .. })
                | (TypeInner::Vector { .. }, TypeInner::Matrix { columns, rows, .. }) => {
                    *columns as u64 * *rows as u64
                }
                _ => n(h),
            }
        }
        Expression::Binary { op: op @ (BinaryOperator::Divide | BinaryOperator::Modulo), .. } => n(h)
            .saturating_mul(match (inner(h).scalar_kind(), op) {
                (Some(ScalarKind::Sint | ScalarKind::Uint), _) => DIVIDE,
                (_, BinaryOperator::Modulo) => 4,
                _ => 1,
            }),
        Expression::Math { fun, arg, .. } => n(*arg).max(n(h)).saturating_mul(hard(*fun)),
        Expression::Binary { .. }
        | Expression::Unary { .. }
        | Expression::Select { .. }
        | Expression::Relational { .. }
        | Expression::As { .. }
        | Expression::Derivative { .. }
        | Expression::Load { .. }
        | Expression::Compose { .. }
        | Expression::Splat { .. }
        | Expression::ZeroValue(_) => n(h),
        Expression::Literal(_)
        | Expression::Constant(_)
        | Expression::Override(_)
        | Expression::FunctionArgument(_)
        | Expression::GlobalVariable(_)
        | Expression::LocalVariable(_)
        | Expression::CallResult(_)
        | Expression::Swizzle { .. }
        | Expression::AccessIndex { .. } => 0,
        _ => 1,
    }
}

fn hard(fun: M) -> u64 {
    match fun {
        M::Sin | M::Cos | M::Tan | M::Sinh | M::Cosh | M::Tanh | M::Asin | M::Acos | M::Atan | M::Atan2 => 4,
        M::Asinh | M::Acosh | M::Atanh | M::Exp | M::Exp2 | M::Log | M::Log2 | M::Pow | M::Sqrt => 4,
        M::InverseSqrt | M::Length | M::Distance | M::Normalize | M::Refract | M::Determinant => 4,
        M::Inverse => 8,
        _ => 1,
    }
}
