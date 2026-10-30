use crate::loops::{MAX_LOOP, Walk};
use naga::{BinaryOperator as Op, Block, Expression, Function, Handle, Literal, MathFunction};
use naga::{LocalVariable, Scalar, ScalarKind, Statement, TypeInner};

impl Walk<'_> {
    pub(crate) fn count(&self, f: &Function, body: &Block, continuing: &Block) -> Option<u64> {
        let guard = body.iter().find(|s| !matches!(s, Statement::Emit(_) | Statement::Call { .. }))?;
        let Statement::If { condition, accept, reject } = guard else { return None };
        let stop = |b: &Block| matches!(b.iter().as_slice(), [Statement::Break]);
        let keeps = match (accept.is_empty(), stop(accept), reject.is_empty(), stop(reject)) {
            (true, _, _, true) => true,
            (_, true, true, _) => false,
            _ => return None,
        };
        let Expression::Binary { op, left, right } = f.expressions[*condition] else { return None };
        let (var, op, limit) = match (self.var(f, left), self.var(f, right)) {
            (Some(v), None) => (v, op, right),
            (None, Some(v)) => (v, flip(op)?, left),
            _ => return None,
        };
        let op = if keeps { op } else { negate(op)? };
        let (step, up) = continuing.iter().find_map(|s| match s {
            Statement::Store { pointer, value } if self.local(f, *pointer) == Some(var) => {
                match f.expressions[*value] {
                    Expression::Binary { op: o @ (Op::Add | Op::Subtract), left, right }
                        if self.var(f, left) == Some(var) =>
                    {
                        Some((self.number(f, right, false)?, o == Op::Add))
                    }
                    _ => None,
                }
            }
            _ => None,
        })?;
        let (mut all, mut inside) = (Vec::new(), Vec::new());
        self.stores(f, &f.body, var, &mut all);
        [body, continuing].into_iter().for_each(|b| self.stores(f, b, var, &mut inside));
        if step < 1.0 || step.fract() != 0.0 || inside.len() != 1 {
            return None;
        }
        let local = &f.local_variables[var];
        let TypeInner::Scalar(scalar) = self.module.types[local.ty].inner else { return None };
        let (lo, hi) = range(scalar)?;
        let fits = |v: &f64| (lo..=hi).contains(v);
        let floor = (lo == 0.0).then_some(0.0);
        let outside = all.iter().filter(|h| !inside.contains(h)).map(|h| literal(&f.expressions[*h]));
        let starts: Option<Vec<f64>> =
            local.init.map(|i| self.number(f, i, !up)).into_iter().chain(outside).collect();
        if starts.as_ref().is_some_and(|s| !s.iter().all(fits)) {
            return None;
        }
        let pick = if up { f64::min } else { f64::max };
        let start = starts.and_then(|s| s.into_iter().reduce(pick));
        let strict = f64::from(u8::from(matches!(op, Op::Less | Op::Greater)));
        let (span, last) = match (up, op) {
            (true, Op::Less | Op::LessEqual) => {
                let limit = self.number(f, limit, true).filter(fits)?;
                (limit - start.or(floor)?, limit + step - strict)
            }
            (false, Op::Greater | Op::GreaterEqual) => {
                let limit = self.number(f, limit, false).or(floor).filter(fits)?;
                (start? - limit, limit - step + strict)
            }
            _ => return None,
        };
        if !(lo..=hi).contains(&last) {
            return None;
        }
        let n = (span / step).floor().max(-1.0) + f64::from(u8::from(strict == 0.0 || span % step != 0.0));
        let n = n.max(0.0) as u64;
        (n <= MAX_LOOP).then_some(n)
    }

    fn stores(&self, f: &Function, b: &Block, var: Handle<LocalVariable>, out: &mut Vec<Handle<Expression>>) {
        for s in b.iter() {
            match s {
                Statement::Store { pointer, value } if self.local(f, *pointer) == Some(var) => {
                    out.push(*value)
                }
                Statement::Block(b) => self.stores(f, b, var, out),
                Statement::If { accept, reject, .. } => {
                    [accept, reject].into_iter().for_each(|b| self.stores(f, b, var, out))
                }
                Statement::Switch { cases, .. } => {
                    cases.iter().for_each(|c| self.stores(f, &c.body, var, out))
                }
                Statement::Loop { body, continuing, .. } => {
                    [body, continuing].into_iter().for_each(|b| self.stores(f, b, var, out))
                }
                _ => {}
            }
        }
    }

    fn local(&self, f: &Function, h: Handle<Expression>) -> Option<Handle<LocalVariable>> {
        match f.expressions[h] {
            Expression::LocalVariable(v) => Some(v),
            _ => None,
        }
    }

    fn var(&self, f: &Function, h: Handle<Expression>) -> Option<Handle<LocalVariable>> {
        match f.expressions[h] {
            Expression::Load { pointer } => self.local(f, pointer),
            _ => None,
        }
    }

    fn number(&self, f: &Function, h: Handle<Expression>, high: bool) -> Option<f64> {
        let global = |h: Handle<Expression>| literal(&self.module.global_expressions[h]);
        match &f.expressions[h] {
            Expression::Constant(c) => global(self.module.constants[*c].init),
            Expression::Override(o) if high => {
                let init =
                    self.module.overrides[*o].init.and_then(|i| literal(&self.module.global_expressions[i]));
                (self.top)(*o).or(init)
            }
            Expression::As { expr, kind, convert: Some(width) } => {
                let to = |v| cast(v, Scalar { kind: *kind, width: *width });
                let (low, top) = (to(self.number(f, *expr, false)?)?, to(self.number(f, *expr, true)?)?);
                Some(if high { top } else { low })
            }
            Expression::Math { fun: MathFunction::Min, arg, arg1: Some(b), .. } if high => {
                let (a, b) = (self.number(f, *arg, high), self.number(f, *b, high));
                a.zip(b).map(|(a, b)| a.min(b)).or(a).or(b)
            }
            e => literal(e),
        }
    }
}

fn range(s: Scalar) -> Option<(f64, f64)> {
    match s {
        Scalar { kind: ScalarKind::Uint, width: 4 } => Some((0.0, f64::from(u32::MAX))),
        Scalar { kind: ScalarKind::Sint, width: 4 } => Some((f64::from(i32::MIN), f64::from(i32::MAX))),
        _ => None,
    }
}

fn cast(v: f64, to: Scalar) -> Option<f64> {
    match to {
        Scalar { kind: ScalarKind::Float, width: 4 } => Some(v).filter(|v| f64::from(*v as f32) == *v),
        Scalar { kind: ScalarKind::Float, width: 8 } => Some(v),
        _ => {
            let (lo, hi) = range(to)?;
            Some(v.trunc()).filter(|t| (lo..=hi).contains(t))
        }
    }
}

fn literal(e: &Expression) -> Option<f64> {
    match e {
        Expression::Literal(l) => match *l {
            Literal::I32(v) => Some(f64::from(v)),
            Literal::U32(v) => Some(f64::from(v)),
            Literal::AbstractInt(v) | Literal::I64(v) => Some(v as f64),
            Literal::U64(v) => Some(v as f64),
            Literal::F32(v) => Some(f64::from(v)),
            Literal::AbstractFloat(v) | Literal::F64(v) => Some(v),
            _ => None,
        },
        _ => None,
    }
}

fn flip(op: Op) -> Option<Op> {
    Some(match op {
        Op::Less => Op::Greater,
        Op::LessEqual => Op::GreaterEqual,
        Op::Greater => Op::Less,
        Op::GreaterEqual => Op::LessEqual,
        _ => return None,
    })
}

fn negate(op: Op) -> Option<Op> {
    Some(match op {
        Op::Less => Op::GreaterEqual,
        Op::LessEqual => Op::Greater,
        Op::Greater => Op::LessEqual,
        Op::GreaterEqual => Op::Less,
        _ => return None,
    })
}
