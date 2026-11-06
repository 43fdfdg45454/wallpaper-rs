use super::value::{Iv, UNIT, V};
use naga::front::Typifier;
use naga::proc::ResolveContext;
use naga::{AddressSpace, Block, Expression as E, Function, Handle, LocalVariable, Module, Statement as S};
use std::collections::HashMap;

const DEEPEST: u32 = 16;
const ROUNDS: usize = 4;

pub(super) type Env = HashMap<Handle<LocalVariable>, V>;

pub(super) struct Walk<'m> {
    pub(super) m: &'m Module,
    pub(super) fx: &'m [[f32; 4]],
    pub(super) rows: HashMap<String, usize>,
    pub(super) units: bool,
    pub(super) reads: Vec<[Iv; 2]>,
    pub(super) bad: bool,
    pub(super) privates: HashMap<Handle<naga::GlobalVariable>, V>,
    depth: u32,
}

pub(super) struct Frame<'f> {
    pub(super) f: &'f Function,
    pub(super) args: Vec<V>,
    pub(super) vals: Vec<Option<V>>,
    pub(super) locals: Env,
    pub(super) ty: Typifier,
    pub(super) uv: Option<u32>,
    ret: Option<V>,
}

fn join(a: &Env, b: &Env) -> Env {
    let keys = a.keys().chain(b.keys());
    keys.map(|k| (*k, a.get(k).zip(b.get(k)).map_or(V::Any, |(x, y)| x.join(*y)))).collect()
}

impl<'m> Walk<'m> {
    pub(super) fn new(m: &'m Module, fx: &'m [[f32; 4]], units: bool) -> Walk<'m> {
        let rows = m.global_variables.iter().find(|(_, g)| g.name.as_deref() == Some("fx"));
        let rows = rows.map(|(_, g)| match &m.types[g.ty].inner {
            naga::TypeInner::Struct { members, .. } => {
                members.iter().filter_map(|s| Some((s.name.clone()?, s.offset as usize / 16))).collect()
            }
            _ => HashMap::new(),
        });
        let (reads, privates) = (Vec::new(), HashMap::new());
        Walk { m, fx, rows: rows.unwrap_or_default(), units, reads, bad: false, privates, depth: 0 }
    }

    pub(super) fn frame<'f>(&self, f: &'f Function, args: Vec<V>, uv: Option<u32>) -> Option<Frame<'f>> {
        let mut ty = Typifier::new();
        let ctx = ResolveContext::with_locals(self.m, &f.local_variables, &f.arguments);
        if let Some((last, _)) = f.expressions.iter().last() {
            ty.grow(last, &f.expressions, &ctx).ok()?;
        }
        let vals = vec![None; f.expressions.len()];
        Some(Frame { f, args, vals, locals: Env::new(), ty, uv, ret: None })
    }

    pub(super) fn run(&mut self, fr: &mut Frame<'_>) -> V {
        for (h, v) in fr.f.local_variables.iter() {
            let init = v.init.map(|i| self.get(fr, i));
            fr.locals.insert(h, init.unwrap_or_else(|| self.zero(v.ty)));
        }
        self.block(fr, &fr.f.body);
        fr.ret.unwrap_or(V::Any)
    }

    fn block(&mut self, fr: &mut Frame<'_>, b: &Block) {
        for s in b.iter() {
            if self.bad {
                return;
            }
            match s {
                S::Emit(r) => r.clone().for_each(|h| fr.vals[h.index()] = Some(self.expr(fr, h))),
                S::Block(b) => self.block(fr, b),
                S::If { condition, accept, reject } => match self.truth(fr, *condition) {
                    Some(true) => self.block(fr, accept),
                    Some(false) => self.block(fr, reject),
                    None => {
                        let start = fr.locals.clone();
                        self.block(fr, accept);
                        let a = std::mem::replace(&mut fr.locals, start);
                        self.block(fr, reject);
                        fr.locals = join(&a, &fr.locals);
                    }
                },
                S::Switch { cases, .. } => {
                    let start = fr.locals.clone();
                    let mut out: Option<Env> = None;
                    for c in cases {
                        fr.locals = start.clone();
                        self.block(fr, &c.body);
                        out = Some(out.map_or_else(|| fr.locals.clone(), |o| join(&o, &fr.locals)));
                    }
                    fr.locals = out.unwrap_or(start);
                }
                S::Loop { body, continuing, .. } => self.repeat(fr, body, continuing),
                S::Store { pointer, value } => {
                    let v = self.get(fr, *value);
                    self.store(fr, *pointer, v);
                }
                S::Call { function, arguments, result } => {
                    let v = self.call(fr, *function, arguments);
                    if let Some(r) = result {
                        fr.vals[r.index()] = Some(v);
                    }
                }
                S::Return { value: Some(v) } => {
                    let v = self.get(fr, *v);
                    fr.ret = Some(fr.ret.map_or(v, |r| r.join(v)));
                }
                S::ImageStore { .. } | S::Atomic { .. } | S::ImageAtomic { .. } => self.bad = true,
                _ => {}
            }
        }
    }

    fn repeat(&mut self, fr: &mut Frame<'_>, body: &Block, continuing: &Block) {
        for _ in 0..ROUNDS {
            let before = fr.locals.clone();
            self.block(fr, body);
            self.block(fr, continuing);
            fr.locals = join(&before, &fr.locals);
            if fr.locals == before {
                return;
            }
        }
        let wide: Vec<_> = fr.locals.keys().map(|k| (*k, self.zero(fr.f.local_variables[*k].ty))).collect();
        for (k, z) in wide {
            fr.locals.insert(k, z.comps().map_or(V::Any, |c| V::wide(c.len())));
        }
        self.block(fr, body);
        self.block(fr, continuing);
    }

    pub(super) fn get(&mut self, fr: &mut Frame<'_>, h: Handle<E>) -> V {
        if let Some(v) = fr.vals[h.index()] {
            return v;
        }
        let v = self.expr(fr, h);
        if !matches!(fr.f.expressions[h], E::Load { .. }) {
            fr.vals[h.index()] = Some(v);
        }
        v
    }

    fn call(&mut self, fr: &mut Frame<'_>, f: Handle<Function>, args: &[Handle<E>]) -> V {
        let vals: Vec<V> = args.iter().map(|a| self.get(fr, *a)).collect();
        let raw = args.iter().position(|a| match fr.f.expressions[*a] {
            E::GlobalVariable(g) => {
                matches!(self.m.global_variables[g].name.as_deref(), Some("input_tex" | "mask_tex"))
            }
            _ => false,
        });
        let name = self.m.functions[f].name.as_deref().unwrap_or_default();
        match (name, vals.first().copied(), raw) {
            ("tap" | "mask_at", Some(at), None) => {
                self.read(at);
                return if name == "tap" { V::wide(4) } else { V::of(&[UNIT]) };
            }
            ("sample", _, Some(0)) if vals.len() == 2 => {
                self.read(vals[1]);
                return V::wide(4);
            }
            (_, _, Some(_)) => self.bad = true,
            ("layer_uv", Some(V::At(false, d)), _) => return V::At(true, self.turn(d, false)),
            ("from_layer", Some(V::At(true, d)), _) => return V::At(false, self.turn(d, true)),
            _ => {}
        }
        if self.bad || self.depth >= DEEPEST {
            self.bad = true;
            return V::Any;
        }
        let Some(mut inner) = self.frame(&self.m.functions[f], vals, None) else {
            self.bad = true;
            return V::Any;
        };
        self.depth += 1;
        let out = self.run(&mut inner);
        self.depth -= 1;
        out
    }

    pub(super) fn read(&mut self, at: V) {
        match at {
            V::At(false, d) => self.reads.push(d),
            _ => self.bad = true,
        }
    }

    pub(super) fn private(&self, g: Handle<naga::GlobalVariable>) -> bool {
        self.m.global_variables[g].space == AddressSpace::Private
    }
}
