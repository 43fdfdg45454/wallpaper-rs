use naga::{Block, Expression, Function, Handle, LocalVariable, Statement};

type Pending = Vec<(Handle<Expression>, Handle<LocalVariable>)>;

pub(crate) fn hoist(f: &mut Function) {
    let mut pending: Pending = f
        .local_variables
        .iter()
        .filter_map(|(v, l)| l.init.filter(|h| !f.expressions[*h].needs_pre_emit()).map(|h| (h, v)))
        .collect();
    if pending.is_empty() {
        return;
    }
    let mut body = std::mem::take(&mut f.body);
    block(f, &mut body, &mut pending);
    f.body = body;
}

fn block(f: &mut Function, b: &mut Block, pending: &mut Pending) {
    for (mut s, span) in std::mem::take(b).span_into_iter() {
        let mut stores = Vec::new();
        match &mut s {
            Statement::Emit(range) => {
                let emitted: Vec<_> = range.clone().collect();
                pending.retain(|(value, local)| {
                    let found = emitted.contains(value);
                    if found {
                        stores.push((*value, *local));
                    }
                    !found
                });
            }
            Statement::Block(inner) => block(f, inner, pending),
            Statement::If { accept, reject, .. } => {
                block(f, accept, pending);
                block(f, reject, pending);
            }
            Statement::Switch { cases, .. } => cases.iter_mut().for_each(|c| block(f, &mut c.body, pending)),
            Statement::Loop { body, continuing, .. } => {
                block(f, body, pending);
                block(f, continuing, pending);
            }
            _ => {}
        }
        b.push(s, span);
        for (value, local) in stores {
            f.local_variables[local].init = None;
            let pointer = f.expressions.append(Expression::LocalVariable(local), span);
            b.push(Statement::Store { pointer, value }, span);
        }
    }
}
