use super::value::{FULL, Iv, V};
use super::walk::{Frame, Walk};
use naga::{BinaryOperator as B, Expression as E, Handle, MathFunction as M, TypeInner as T};

pub(super) enum Place {
    Local(Handle<naga::LocalVariable>, Option<usize>),
    Private(Handle<naga::GlobalVariable>),
    Fx,
    List(usize, usize),
    Rows(usize, usize, Option<usize>),
    Other,
}

impl Walk<'_> {
    pub(super) fn zero(&self, ty: Handle<naga::Type>) -> V {
        match &self.m.types[ty].inner {
            naga::TypeInner::Scalar(_) => V::of(&[Iv::at(0.0)]),
            naga::TypeInner::Vector { size, .. } => V::of(&[Iv::at(0.0); 4][..*size as usize]),
            _ => V::Any,
        }
    }

    fn fx(&self) -> Option<(&naga::GlobalVariable, &[naga::StructMember])> {
        let (_, g) = self.m.global_variables.iter().find(|(_, g)| g.name.as_deref() == Some("fx"))?;
        match &self.m.types[g.ty].inner {
            T::Struct { members, .. } => Some((g, members)),
            _ => None,
        }
    }

    fn place(&mut self, fr: &mut Frame<'_>, h: Handle<E>) -> Place {
        match fr.f.expressions[h] {
            E::LocalVariable(l) => Place::Local(l, None),
            E::GlobalVariable(g) if self.m.global_variables[g].name.as_deref() == Some("fx") => Place::Fx,
            E::GlobalVariable(g) if self.private(g) => Place::Private(g),
            E::AccessIndex { base, index } => match (self.place(fr, base), index as usize) {
                (Place::Fx, k) => {
                    let Some(s) = self.fx().and_then(|(_, m)| m.get(k).cloned()) else { return Place::Other };
                    let row = s.offset as usize / 16;
                    match self.m.types[s.ty].inner {
                        T::Array { size: naga::ArraySize::Constant(n), .. } => {
                            Place::List(row, n.get() as usize)
                        }
                        T::Vector { .. } => Place::Rows(row, 1, None),
                        T::Scalar(_) => Place::Rows(row, 1, Some(s.offset as usize % 16 / 4)),
                        _ => Place::Other,
                    }
                }
                (Place::List(first, n), k) if k < n => Place::Rows(first + k, 1, None),
                (Place::Rows(r, c, None), k) => Place::Rows(r, c, Some(k)),
                (Place::Local(l, None), k) => Place::Local(l, Some(k)),
                _ => Place::Other,
            },
            E::Access { base, index } => match self.place(fr, base) {
                Place::List(first, n) => {
                    let i = self.value(fr, index).comps().map_or(FULL, |c| c[0]);
                    let (lo, hi) = (i.0.floor().max(0.0), i.1.ceil().min(n as f64 - 1.0));
                    if lo.is_nan() || hi.is_nan() || lo > hi {
                        Place::Other
                    } else {
                        Place::Rows(first + lo as usize, (hi - lo) as usize + 1, None)
                    }
                }
                _ => Place::Other,
            },
            _ => Place::Other,
        }
    }

    pub(super) fn load(&mut self, fr: &mut Frame<'_>, pointer: Handle<E>, n: usize) -> V {
        match self.place(fr, pointer) {
            Place::Local(l, None) => fr.locals.get(&l).copied().unwrap_or(V::Any),
            Place::Local(l, Some(k)) => {
                let v = fr.locals.get(&l).copied().unwrap_or(V::Any);
                self.num(v, 4).comps().and_then(|c| c.get(k).copied()).map_or(V::wide(1), |c| V::of(&[c]))
            }
            Place::Private(g) => {
                self.privates.get(&g).copied().unwrap_or_else(|| self.zero(self.m.global_variables[g].ty))
            }
            Place::Rows(first, count, comp) if first + count <= self.fx.len() => {
                let rows = &self.fx[first..first + count];
                let at =
                    |k: usize| rows.iter().map(|r| Iv::at(f64::from(r[k]))).reduce(Iv::join).unwrap_or(FULL);
                match comp {
                    Some(k) if k < 4 => V::of(&[at(k)]),
                    _ => V::of(&[at(0), at(1), at(2), at(3)]),
                }
            }
            _ => V::wide(n),
        }
    }

    pub(super) fn store(&mut self, fr: &mut Frame<'_>, pointer: Handle<E>, v: V) {
        match self.place(fr, pointer) {
            Place::Local(l, None) => {
                fr.locals.insert(l, v);
            }
            Place::Local(l, Some(k)) => {
                let z = self.zero(fr.f.local_variables[l].ty);
                let cur = fr.locals.get(&l).copied().unwrap_or(z);
                let mut c = self
                    .num(cur, z.comps().map_or(1, <[Iv]>::len))
                    .comps()
                    .map(<[Iv]>::to_vec)
                    .unwrap_or_default();
                let x = self.num(v, 1).comps().map_or(FULL, |c| c[0]);
                if let Some(slot) = c.get_mut(k) {
                    *slot = x;
                }
                fr.locals.insert(l, V::of(&c));
            }
            Place::Private(g) => {
                self.privates.insert(g, v);
            }
            _ => {}
        }
    }

    pub(super) fn turn(&self, d: [Iv; 2], back: bool) -> [Iv; 2] {
        let row = |n: &str| {
            self.rows.get(n).and_then(|r| self.fx.get(*r)).map(|r| [f64::from(r[0]), f64::from(r[1])])
        };
        let (Some([a, b]), Some([c, e])) = (row("mask_m0"), row("mask_m1")) else { return [FULL; 2] };
        let det = a * e - b * c;
        let [p, q, r, s] = if back { [e / det, -b / det, -c / det, a / det] } else { [a, b, c, e] };
        if !det.is_finite() || det.abs() < 1e-12 {
            return [FULL; 2];
        }
        let mix = |x: f64, y: f64| Iv::binary(B::Add, d[0].scaled(x), d[1].scaled(y));
        [mix(p, q), mix(r, s)]
    }

    pub(super) fn math(
        &mut self,
        fr: &mut Frame<'_>,
        fun: M,
        (a, b, c): (Handle<E>, Option<Handle<E>>, Option<Handle<E>>),
        n: usize,
    ) -> V {
        let x = self.value(fr, a);
        let y = b.map_or(V::of(&[FULL]), |h| self.value(fr, h));
        let z = c.map_or(V::of(&[FULL]), |h| self.value(fr, h));
        let (Some(xs), Some(ys), Some(zs)) = (x.comps(), y.comps(), z.comps()) else { return V::wide(n) };
        let least = |i: &Iv| {
            if i.0 > 0.0 {
                i.0
            } else if i.1 < 0.0 {
                -i.1
            } else {
                0.0
            }
        };
        let size = |v: &[Iv]| {
            let s = |f: &dyn Fn(&Iv) -> f64| v.iter().map(|i| f(i).powi(2)).sum::<f64>().sqrt();
            Iv(s(&least), s(&|i: &Iv| i.most()))
        };
        match fun {
            M::Dot if xs.len() == ys.len() => V::of(&[xs
                .iter()
                .zip(ys)
                .fold(Iv::at(0.0), |s, (p, q)| Iv::binary(B::Add, s, Iv::binary(B::Multiply, *p, *q)))]),
            M::Length => V::of(&[size(xs)]),
            M::Distance => V::each(&x, &y, |p, q| Iv::binary(B::Subtract, p, q))
                .comps()
                .map_or(V::wide(1), |d| V::of(&[size(d)])),
            M::Cross | M::Outer | M::Determinant | M::Inverse | M::Transpose | M::Modf | M::Frexp => {
                V::wide(n)
            }
            _ => {
                let k = xs.len().max(ys.len()).max(zs.len()).min(n.max(1));
                let at = |v: &[Iv], i: usize| v[i.min(v.len() - 1)];
                V::of(&(0..k).map(|i| Iv::math(fun, at(xs, i), at(ys, i), at(zs, i))).collect::<Vec<_>>())
            }
        }
    }
}
