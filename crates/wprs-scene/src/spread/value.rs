use naga::{BinaryOperator as B, MathFunction as M};

const INF: f64 = f64::INFINITY;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct Iv(pub(super) f64, pub(super) f64);

pub(super) const FULL: Iv = Iv(-INF, INF);
pub(super) const UNIT: Iv = Iv(0.0, 1.0);
pub(super) const SIGNED: Iv = Iv(-1.0, 1.0);

fn fix(v: f64, or: f64) -> f64 {
    if v.is_nan() { or } else { v }
}

impl Iv {
    pub(super) fn at(v: f64) -> Iv {
        Iv(v, v)
    }

    pub(super) fn join(self, o: Iv) -> Iv {
        Iv(self.0.min(o.0), self.1.max(o.1))
    }

    fn from(vals: [f64; 4]) -> Iv {
        let lo = vals.iter().map(|v| fix(*v, -INF)).fold(INF, f64::min);
        Iv(lo, vals.iter().map(|v| fix(*v, INF)).fold(-INF, f64::max))
    }

    fn mono(self, f: impl Fn(f64) -> f64) -> Iv {
        Iv(fix(f(self.0), -INF), fix(f(self.1), INF))
    }

    fn truth(yes: bool, no: bool) -> Iv {
        if yes {
            Iv::at(1.0)
        } else if no {
            Iv::at(0.0)
        } else {
            UNIT
        }
    }

    pub(super) fn scaled(self, k: f64) -> Iv {
        self.mul(Iv::at(k))
    }

    pub(super) fn most(self) -> f64 {
        self.0.abs().max(self.1.abs())
    }

    fn mul(self, o: Iv) -> Iv {
        Iv::from([self.0 * o.0, self.0 * o.1, self.1 * o.0, self.1 * o.1])
    }

    fn div(self, o: Iv) -> Iv {
        if o.0 <= 0.0 && o.1 >= 0.0 { FULL } else { self.mul(Iv(1.0 / o.1, 1.0 / o.0)) }
    }

    pub(super) fn binary(op: B, a: Iv, b: Iv) -> Iv {
        match op {
            B::Add => Iv(fix(a.0 + b.0, -INF), fix(a.1 + b.1, INF)),
            B::Subtract => Iv(fix(a.0 - b.1, -INF), fix(a.1 - b.0, INF)),
            B::Multiply => a.mul(b),
            B::Divide => a.div(b),
            B::Modulo => Iv(-b.most(), b.most()),
            B::Less => Iv::truth(a.1 < b.0, a.0 >= b.1),
            B::LessEqual => Iv::truth(a.1 <= b.0, a.0 > b.1),
            B::Greater => Iv::truth(a.0 > b.1, a.1 <= b.0),
            B::GreaterEqual => Iv::truth(a.0 >= b.1, a.1 < b.0),
            B::And | B::InclusiveOr | B::ExclusiveOr | B::ShiftLeft | B::ShiftRight => FULL,
            _ => UNIT,
        }
    }

    pub(super) fn math(fun: M, a: Iv, b: Iv, c: Iv) -> Iv {
        match fun {
            M::Abs => Iv(if a.0 <= 0.0 && a.1 >= 0.0 { 0.0 } else { a.0.abs().min(a.1.abs()) }, a.most()),
            M::Min => Iv(a.0.min(b.0), a.1.min(b.1)),
            M::Max => Iv(a.0.max(b.0), a.1.max(b.1)),
            M::Clamp => Iv(a.0.max(b.0).min(c.0), a.1.max(b.1).min(c.1)),
            M::Saturate => Iv(a.0.clamp(0.0, 1.0), a.1.clamp(0.0, 1.0)),
            M::Sin | M::Cos | M::Tanh | M::Sign | M::Normalize => SIGNED,
            M::Atan => Iv(-1.6, 1.6),
            M::Atan2 => Iv(-3.2, 3.2),
            M::Radians => a.mul(Iv::at(std::f64::consts::PI / 180.0)),
            M::Degrees => a.mul(Iv::at(180.0 / std::f64::consts::PI)),
            M::Floor | M::Round | M::Trunc | M::Ceil => Iv(a.0 - 1.0, a.1 + 1.0),
            M::Fract | M::Step | M::SmoothStep => UNIT,
            M::Exp => a.mono(f64::exp),
            M::Exp2 => a.mono(f64::exp2),
            M::Sqrt if a.0 >= 0.0 => a.mono(f64::sqrt),
            M::Log if a.0 > 0.0 => a.mono(f64::ln),
            M::Log2 if a.0 > 0.0 => a.mono(f64::log2),
            M::Pow if a.0 >= 0.0 && a.1 <= 1.0 && b.0 >= 0.0 => UNIT,
            M::Pow if a.0 >= 0.0 && b.0 == b.1 && b.0 >= 0.0 => a.mono(|v| v.powf(b.0)),
            M::Fma => Iv::binary(B::Add, a.mul(b), c),
            M::Mix if c.0 >= 0.0 && c.1 <= 1.0 => a.join(b),
            M::Mix => Iv::binary(B::Add, a, Iv::binary(B::Subtract, b, a).mul(c)),
            _ => FULL,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum V {
    Num([Iv; 4], usize),
    At(bool, [Iv; 2]),
    Any,
}

impl V {
    pub(super) fn of(c: &[Iv]) -> V {
        let mut v = [FULL; 4];
        v[..c.len().min(4)].copy_from_slice(&c[..c.len().min(4)]);
        if c.is_empty() || c.len() > 4 { V::Any } else { V::Num(v, c.len()) }
    }

    pub(super) fn wide(n: usize) -> V {
        V::of(&[FULL; 4][..n.clamp(1, 4)])
    }

    pub(super) fn comps(&self) -> Option<&[Iv]> {
        match self {
            V::Num(v, n) => Some(&v[..*n]),
            _ => None,
        }
    }

    pub(super) fn join(self, o: V) -> V {
        match (self, o) {
            (V::Num(a, n), V::Num(b, m)) if n == m => {
                V::of(&(0..n).map(|k| a[k].join(b[k])).collect::<Vec<_>>())
            }
            (V::At(l, a), V::At(k, b)) if l == k => V::At(l, [a[0].join(b[0]), a[1].join(b[1])]),
            _ => V::Any,
        }
    }

    pub(super) fn each(a: &V, b: &V, f: impl Fn(Iv, Iv) -> Iv) -> V {
        let (Some(x), Some(y)) = (a.comps(), b.comps()) else { return V::Any };
        let n = x.len().max(y.len());
        if x.len() != y.len() && x.len() != 1 && y.len() != 1 {
            return V::Any;
        }
        V::of(&(0..n).map(|k| f(x[k.min(x.len() - 1)], y[k.min(y.len() - 1)])).collect::<Vec<_>>())
    }

    pub(super) fn binary(op: B, a: &V, b: &V) -> V {
        let shift = |d: &[Iv; 2], n: &V, sub: bool| {
            let n = n.comps().filter(|c| c.len() <= 2)?;
            let at = |k: usize| n[k.min(n.len() - 1)];
            let op = if sub { B::Subtract } else { B::Add };
            Some([Iv::binary(op, d[0], at(0)), Iv::binary(op, d[1], at(1))])
        };
        match (op, a, b) {
            (B::Add | B::Subtract, V::At(l, d), n @ V::Num(..)) => {
                shift(d, n, op == B::Subtract).map_or(V::Any, |d| V::At(*l, d))
            }
            (B::Add, n @ V::Num(..), V::At(l, d)) => shift(d, n, false).map_or(V::Any, |d| V::At(*l, d)),
            (_, V::Num(..), V::Num(..)) => V::each(a, b, |x, y| Iv::binary(op, x, y)),
            _ => V::Any,
        }
    }
}
