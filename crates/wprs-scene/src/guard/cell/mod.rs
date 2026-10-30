use naga::{Binding, Block, BuiltIn, Expression as E, Function, FunctionArgument};
use naga::{Handle, Literal, Module, Scalar, ShaderStage, Span, Statement, Type, TypeInner};

mod build;

const AT: Span = Span::UNDEFINED;

struct Wrap<'a> {
    types: &'a naga::UniqueArena<Type>,
    user: Handle<Function>,
    with: Handle<Function>,
    args: Vec<FunctionArgument>,
    member: Option<(u32, u32)>,
    f: Function,
    body: Block,
}

fn position(b: &Option<Binding>) -> bool {
    matches!(b, Some(Binding::BuiltIn(BuiltIn::Position { .. })))
}

pub(crate) fn cell(module: &mut Module) -> Result<(), String> {
    let found = module.functions.iter().find(|(_, f)| f.name.as_deref() == Some("fx_cell"));
    let with = found.map(|(h, _)| h).ok_or("its guarded shader lacks fx_cell")?;
    let u32 = module.types.insert(Type { name: None, inner: TypeInner::Scalar(Scalar::U32) }, AT);
    for at in 0..module.entry_points.len() {
        if module.entry_points[at].stage != ShaderStage::Vertex {
            continue;
        }
        let mut inner = std::mem::take(&mut module.entry_points[at].function);
        let result = inner.result.clone().ok_or("its vertex entry returns nothing")?;
        let member = match &module.types[result.ty].inner {
            _ if position(&result.binding) => None,
            TypeInner::Struct { members, .. } => {
                let m = members.iter().position(|m| position(&m.binding)).ok_or("has no vertex position")?;
                Some((m as u32, members.len() as u32))
            }
            _ => return Err("its vertex entry returns no position".into()),
        };
        let args = inner.arguments.clone();
        inner.arguments.iter_mut().for_each(|a| a.binding = None);
        inner.result.iter_mut().for_each(|r| r.binding = None);
        inner.name = Some(format!("fx_vertex_{at}"));
        let user = module.functions.append(inner, AT);
        let f = Function { result: Some(result), ..Function::default() };
        let w = Wrap { types: &module.types, user, with, args, member, f, body: Block::new() };
        module.entry_points[at].function = w.build(u32);
    }
    Ok(())
}

impl Wrap<'_> {
    fn add(&mut self, e: E) -> Handle<E> {
        self.f.expressions.append(e, AT)
    }

    fn emit(&mut self, start: usize) {
        if self.f.expressions.len() > start {
            self.body.push(Statement::Emit(self.f.expressions.range_from(start)), AT);
        }
    }

    fn value(&mut self, ty: Handle<Type>, binding: &Option<Binding>, v: Handle<E>) -> Handle<E> {
        match binding {
            Some(Binding::BuiltIn(BuiltIn::VertexIndex)) => v,
            Some(Binding::BuiltIn(BuiltIn::InstanceIndex)) => self.add(E::Literal(Literal::U32(0))),
            _ => match &self.types[ty].inner {
                TypeInner::Struct { members, .. } => {
                    let members = members.clone();
                    let components = members.iter().map(|m| self.value(m.ty, &m.binding, v)).collect();
                    let start = self.f.expressions.len();
                    let made = self.add(E::Compose { ty, components });
                    self.emit(start);
                    made
                }
                _ => self.add(E::ZeroValue(ty)),
            },
        }
    }

    fn call(&mut self, function: Handle<Function>, arguments: Vec<Handle<E>>) -> Handle<E> {
        let result = self.add(E::CallResult(function));
        self.body.push(Statement::Call { function, arguments, result: Some(result) }, AT);
        result
    }
}
