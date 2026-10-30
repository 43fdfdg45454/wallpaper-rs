use super::{AT, Wrap};
use naga::{BinaryOperator as Op, Binding, BuiltIn, Expression as E, Function, FunctionArgument};
use naga::{FunctionResult, Handle, Literal, Statement, Type};

impl Wrap<'_> {
    pub(super) fn build(mut self, u32: Handle<Type>) -> Function {
        let binding = Some(Binding::BuiltIn(BuiltIn::VertexIndex));
        self.f.arguments = vec![FunctionArgument { name: Some("i".into()), ty: u32, binding }];
        let i = self.add(E::FunctionArgument(0));
        let n: Vec<Handle<E>> = (0..3).map(|n| self.add(E::Literal(Literal::U32(n)))).collect();
        let three = self.add(E::Literal(Literal::U32(3)));
        let start = self.f.expressions.len();
        let k = self.add(E::Binary { op: Op::Modulo, left: i, right: three });
        let first = self.add(E::Binary { op: Op::Subtract, left: i, right: k });
        let at: Vec<Handle<E>> =
            n.iter().map(|o| self.add(E::Binary { op: Op::Add, left: first, right: *o })).collect();
        self.emit(start);
        let mut out = Vec::new();
        for v in at {
            let args = self.args.clone();
            let values = args.iter().map(|a| self.value(a.ty, &a.binding, v)).collect();
            out.push(self.call(self.user, values));
        }
        let start = self.f.expressions.len();
        let part = |w: &mut Self, r: Handle<E>, index: u32| w.add(E::AccessIndex { base: r, index });
        let corners: Vec<Handle<E>> = match self.member {
            Some((m, _)) => out.iter().map(|r| part(&mut self, *r, m)).collect(),
            None => out.clone(),
        };
        self.emit(start);
        let placed = self.call(self.with, vec![corners[0], corners[1], corners[2], k]);
        let value = match self.member {
            None => placed,
            Some((m, count)) => {
                let start = self.f.expressions.len();
                let picks = [n[0], n[1]].map(|o| self.add(E::Binary { op: Op::Equal, left: k, right: o }));
                let mut components = Vec::new();
                for j in 0..count {
                    if j == m {
                        components.push(placed);
                        continue;
                    }
                    let [a, b, c] = [0, 1, 2].map(|r| part(&mut self, out[r], j));
                    let reject = self.add(E::Select { condition: picks[1], accept: b, reject: c });
                    components.push(self.add(E::Select { condition: picks[0], accept: a, reject }));
                }
                let ty = self.f.result.as_ref().map(|r: &FunctionResult| r.ty).unwrap_or(u32);
                let made = self.add(E::Compose { ty, components });
                self.emit(start);
                made
            }
        };
        self.body.push(Statement::Return { value: Some(value) }, AT);
        self.f.body = self.body;
        self.f
    }
}
