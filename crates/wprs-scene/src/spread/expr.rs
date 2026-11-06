use super::value::{FULL, Iv, UNIT, V};
use super::walk::{Frame, Walk};
use naga::{BinaryOperator as B, Expression as E, Handle, Literal as L, TypeInner as T};

fn literal(l: &L) -> Option<f64> {
    Some(match *l {
        L::F64(v) | L::AbstractFloat(v) => v,
        L::F32(v) => f64::from(v),
        L::U32(v) => f64::from(v),
        L::I32(v) => f64::from(v),
        L::AbstractInt(v) => v as f64,
        L::Bool(b) => f64::from(u8::from(b)),
        _ => return None,
    })
}

impl Walk<'_> {
    pub(super) fn truth(&mut self, fr: &mut Frame<'_>, h: Handle<E>) -> Option<bool> {
        match self.value(fr, h).comps()?.iter().copied().reduce(Iv::join)? {
            Iv(lo, _) if lo >= 1.0 => Some(true),
            Iv(_, hi) if hi <= 0.0 => Some(false),
            _ => None,
        }
    }

    fn constant(&self, h: Handle<E>) -> Option<Vec<Iv>> {
        match &self.m.global_expressions[h] {
            E::Literal(l) => Some(vec![Iv::at(literal(l)?)]),
            E::Compose { components, .. } => {
                let parts: Option<Vec<Vec<Iv>>> = components.iter().map(|c| self.constant(*c)).collect();
                Some(parts?.concat())
            }
            E::Splat { size, value } => Some(vec![self.constant(*value)?[0]; *size as usize]),
            _ => None,
        }
    }

    pub(super) fn width(&self, fr: &Frame<'_>, h: Handle<E>) -> usize {
        match fr.ty.get(h, &self.m.types) {
            T::Scalar(_) => 1,
            T::Vector { size, .. } => *size as usize,
            _ => 0,
        }
    }

    pub(super) fn num(&self, v: V, n: usize) -> V {
        match v {
            V::Num(..) => v,
            V::At(false, d) => V::of(&[Iv::binary(B::Add, UNIT, d[0]), Iv::binary(B::Add, UNIT, d[1])]),
            _ => V::wide(n),
        }
    }

    pub(super) fn value(&mut self, fr: &mut Frame<'_>, h: Handle<E>) -> V {
        let (v, n) = (self.get(fr, h), self.width(fr, h));
        self.num(v, n)
    }

    pub(super) fn expr(&mut self, fr: &mut Frame<'_>, h: Handle<E>) -> V {
        let n = self.width(fr, h);
        let wide = V::wide(n);
        let comps = |v: V| v.comps().map(<[Iv]>::to_vec);
        match &fr.f.expressions[h] {
            E::Literal(l) => literal(l).map_or(wide, |v| V::of(&[Iv::at(v)])),
            E::Constant(c) => self.constant(self.m.constants[*c].init).map_or(wide, |c| V::of(&c)),
            E::ZeroValue(_) if n > 0 => V::of(&vec![Iv::at(0.0); n]),
            E::Compose { components, .. } => {
                let mut out = Vec::new();
                for c in components.clone() {
                    match comps(self.value(fr, c)) {
                        Some(c) => out.extend(c),
                        None => return wide,
                    }
                }
                if out.len() == n { V::of(&out) } else { wide }
            }
            E::Splat { size, value } => {
                comps(self.value(fr, *value)).map_or(wide, |c| V::of(&vec![c[0]; *size as usize]))
            }
            E::Swizzle { size, vector, pattern } => {
                let v = self.get(fr, *vector);
                let k = *size as usize;
                if let V::At(l, d) = v
                    && k == 2
                    && pattern[0] as usize == 0
                    && pattern[1] as usize == 1
                {
                    return V::At(l, d);
                }
                let c = comps(self.value(fr, *vector)).unwrap_or_default();
                V::of(
                    &pattern[..k]
                        .iter()
                        .map(|p| c.get(*p as usize).copied().unwrap_or(FULL))
                        .collect::<Vec<_>>(),
                )
            }
            E::AccessIndex { base, index } => {
                if fr.uv == Some(*index) && matches!(fr.f.expressions[*base], E::FunctionArgument(0)) {
                    return V::At(false, [Iv::at(0.0); 2]);
                }
                let c = comps(self.value(fr, *base)).unwrap_or_default();
                c.get(*index as usize).map_or(wide, |c| V::of(&[*c]))
            }
            E::Access { base, .. } => {
                let c = comps(self.value(fr, *base)).unwrap_or_default();
                c.iter().copied().reduce(Iv::join).map_or(wide, |c| V::of(&[c]))
            }
            E::FunctionArgument(k) => fr.args.get(*k as usize).copied().unwrap_or(V::Any),
            E::Load { pointer } => self.load(fr, *pointer, n),
            E::ImageSample { image, coordinate, .. } | E::ImageLoad { image, coordinate, .. } => {
                if let E::GlobalVariable(g) = fr.f.expressions[*image]
                    && matches!(self.m.global_variables[g].name.as_deref(), Some("input_tex" | "mask_tex"))
                {
                    let at = self.get(fr, *coordinate);
                    self.read(at);
                    return V::wide(4);
                }
                V::of(&[if self.units { UNIT } else { FULL }; 4])
            }
            E::Unary { op: naga::UnaryOperator::Negate, expr } => comps(self.value(fr, *expr))
                .map_or(wide, |c| V::of(&c.iter().map(|i| Iv(-i.1, -i.0)).collect::<Vec<_>>())),
            E::Binary { op, left, right } => {
                let keep = |v: V| matches!(v, V::At(..)) && matches!(op, B::Add | B::Subtract);
                let (a, b) = (self.get(fr, *left), self.get(fr, *right));
                let a = if keep(a) { a } else { self.value(fr, *left) };
                let b = if keep(b) { b } else { self.value(fr, *right) };
                match V::binary(*op, &a, &b) {
                    V::Any => wide,
                    v => v,
                }
            }
            E::Select { condition, accept, reject } => {
                let (a, b) = match self.truth(fr, *condition) {
                    Some(true) => (self.get(fr, *accept), self.get(fr, *accept)),
                    Some(false) => (self.get(fr, *reject), self.get(fr, *reject)),
                    None => (self.get(fr, *accept), self.get(fr, *reject)),
                };
                match a.join(b) {
                    V::Any => self.value(fr, *accept).join(self.value(fr, *reject)),
                    v => v,
                }
            }
            E::Math { fun, arg, arg1, arg2, .. } => self.math(fr, *fun, (*arg, *arg1, *arg2), n),
            E::As { expr, .. } => self.value(fr, *expr),
            _ => wide,
        }
    }
}
