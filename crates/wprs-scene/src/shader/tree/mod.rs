use crate::shape::{MAX_ARGUMENTS, MAX_BRACKETS, MAX_OPERANDS};
use naga::{Block, Expression as E, Handle, Module, Statement};
use operands::operands;

mod operands;
#[cfg(test)]
mod tests;

pub(crate) fn tree(module: &Module) -> Result<(), String> {
    let all = module.functions.iter().map(|(_, f)| f).chain(module.entry_points.iter().map(|e| &e.function));
    let mut arguments = 0usize;
    for f in all {
        let mut deep = vec![0usize; f.expressions.len()];
        for (h, e) in f.expressions.iter() {
            let mut most = 0;
            operands(e, &mut |o: Handle<E>| {
                if !f.named_expressions.contains_key(&o) {
                    most = most.max(deep.get(o.index()).copied().unwrap_or(0));
                }
            });
            deep[h.index()] = most + usize::from(!leaf(e));
            if deep[h.index()] > MAX_OPERANDS {
                return Err(format!("its shader has an expression more than {MAX_OPERANDS} operations deep"));
            }
            arguments += match e {
                E::Compose { components, .. } => components.len(),
                E::Math { arg1, arg2, arg3, .. } => {
                    1 + [arg1, arg2, arg3].iter().filter(|a| a.is_some()).count()
                }
                _ => 0,
            };
        }
        arguments += nest(&f.body, 1)?;
    }
    if arguments > MAX_ARGUMENTS {
        return Err(format!(
            "its shader passes more than {MAX_ARGUMENTS} arguments to calls and constructors"
        ));
    }
    Ok(())
}

fn leaf(e: &E) -> bool {
    matches!(
        e,
        E::Literal(_)
            | E::Constant(_)
            | E::Override(_)
            | E::ZeroValue(_)
            | E::FunctionArgument(_)
            | E::GlobalVariable(_)
            | E::LocalVariable(_)
            | E::CallResult(_)
            | E::Load { .. }
    )
}

fn nest(b: &Block, depth: usize) -> Result<usize, String> {
    if depth > MAX_BRACKETS {
        return Err(format!("its shader nests more than {MAX_BRACKETS} blocks"));
    }
    let mut arguments = 0;
    for s in b.iter() {
        arguments += match s {
            Statement::Block(b) => nest(b, depth + 1)?,
            Statement::If { accept, reject, .. } => nest(accept, depth + 1)? + nest(reject, depth + 1)?,
            Statement::Switch { cases, .. } => {
                cases.iter().map(|c| nest(&c.body, depth + 1)).sum::<Result<usize, String>>()?
            }
            Statement::Loop { body, continuing, .. } => nest(body, depth + 1)? + nest(continuing, depth + 1)?,
            Statement::Call { arguments, .. } => arguments.len(),
            _ => 0,
        };
    }
    Ok(arguments)
}
