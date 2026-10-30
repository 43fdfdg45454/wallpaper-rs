use naga::{Expression as E, Handle, SampleLevel};

pub(super) fn operands(e: &E, out: &mut impl FnMut(Handle<E>)) {
    let mut put = |h: &Handle<E>| out(*h);
    match e {
        E::Compose { components, .. } => components.iter().for_each(&mut put),
        E::Access { base, index } => [base, index].into_iter().for_each(&mut put),
        E::AccessIndex { base, .. } => put(base),
        E::Splat { value, .. } => put(value),
        E::Swizzle { vector, .. } => put(vector),
        E::Load { pointer } => put(pointer),
        E::ImageSample { coordinate, array_index, depth_ref, level, .. } => {
            [Some(coordinate), array_index.as_ref(), depth_ref.as_ref()]
                .into_iter()
                .flatten()
                .for_each(&mut put);
            match level {
                SampleLevel::Exact(h) | SampleLevel::Bias(h) => put(h),
                SampleLevel::Gradient { x, y } => [x, y].into_iter().for_each(&mut put),
                _ => {}
            }
        }
        E::ImageLoad { coordinate, array_index, sample, level, .. } => {
            [Some(coordinate), array_index.as_ref(), sample.as_ref(), level.as_ref()]
                .into_iter()
                .flatten()
                .for_each(&mut put)
        }
        E::Unary { expr, .. } | E::Derivative { expr, .. } | E::As { expr, .. } | E::ArrayLength(expr) => {
            put(expr)
        }
        E::Binary { left, right, .. } => [left, right].into_iter().for_each(&mut put),
        E::Select { condition, accept, reject } => [condition, accept, reject].into_iter().for_each(&mut put),
        E::Relational { argument, .. } => put(argument),
        E::Math { arg, arg1, arg2, arg3, .. } => {
            [Some(arg), arg1.as_ref(), arg2.as_ref(), arg3.as_ref()].into_iter().flatten().for_each(&mut put)
        }
        _ => {}
    }
}
