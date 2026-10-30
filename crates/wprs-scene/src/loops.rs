pub(crate) use crate::loops::entries::{Walk, work};
use crate::spill::{Reach, root, through, touches};
use crate::tally::Work;
use crate::weigh::{SAMPLE, sized, weight};
use naga::valid::FunctionInfo;
use naga::{Block, Expression, Function, Handle, Statement};

mod entries;

pub const MAX_LOOP: u64 = 1024;
pub const MAX_WORK: u64 = 1 << 16;
pub const MAX_SHADER_NODES: usize = 1 << 15;
pub const MAX_OPS: u64 = 1 << 20;
pub const MAX_INLINED: u64 = 1 << 15;

const COUNTED: &str = "has a loop without a counter that a literal limits and a literal steps, in whole numbers \
     that stay in range; write for (var i = 0u; i < 8u; i++)";

impl Walk<'_> {
    pub(crate) fn block(&mut self, at: (&Function, &FunctionInfo), b: &Block) -> Result<Work, String> {
        self.run(at, b.iter().as_slice())
    }

    fn run(&mut self, (f, fi): (&Function, &FunctionInfo), b: &[Statement]) -> Result<Work, String> {
        let mut sum = Work::default();
        for s in b {
            let one = match s {
                Statement::Emit(range) => range.clone().fold(Work::default(), |w, h| {
                    let sampled = matches!(
                        f.expressions[h],
                        Expression::ImageSample { .. } | Expression::ImageLoad { .. }
                    );
                    let size = weight(self.module, fi, f, h);
                    let ops = size.saturating_add(self.toll(touches((self.module, fi, &self.written), f, h)));
                    w.then(Work { ops, reads: u64::from(sampled), size })
                }),
                Statement::Store { pointer, value } => {
                    let size = sized(self.module, fi, *value);
                    let ops = size.saturating_add(self.toll(through(
                        (self.module, fi, &self.written),
                        f,
                        *pointer,
                    )));
                    Work { ops, reads: 0, size }
                }
                Statement::Block(b) => self.block((f, fi), b)?,
                Statement::If { accept, reject, .. } => {
                    self.block((f, fi), accept)?.or(self.block((f, fi), reject)?)
                }
                Statement::Switch { cases, .. } => cases
                    .iter()
                    .map(|c| self.block((f, fi), &c.body))
                    .try_fold(Work::default(), |m, c| c.map(|c| m.or(c)))?,
                Statement::Loop { body, continuing, break_if } if break_if.is_none() => {
                    let n = self.count(f, body, continuing).ok_or(COUNTED)?;
                    let all = body.iter().as_slice();
                    let guard =
                        all.iter().position(|s| !matches!(s, Statement::Emit(_) | Statement::Call { .. }));
                    let first = self.run((f, fi), &all[..guard.unwrap_or(0)])?;
                    self.block((f, fi), body)?
                        .then(self.block((f, fi), continuing)?)
                        .times(n)
                        .then(Work { size: 0, ..first })
                }
                Statement::Loop { .. } => return Err(COUNTED.into()),
                Statement::Call { function, arguments, .. } => {
                    let local = |a: &Handle<Expression>| {
                        matches!(f.expressions[root(f, *a)], Expression::LocalVariable(_))
                            || through((self.module, fi, &self.written), f, *a) != Reach::Plain
                    };
                    if arguments.iter().any(local) {
                        return Err("passes a pointer to a local variable to a function".into());
                    }
                    self.call(*function)?
                }
                _ => Work::default(),
            };
            sum = sum.then(Work::step(1)).then(one);
        }
        Ok(sum)
    }

    fn toll(&self, reach: Reach) -> u64 {
        match reach {
            Reach::Plain => 0,
            Reach::Fetch => SAMPLE,
            Reach::Spill => self.toll,
        }
    }

    pub(crate) fn call(&mut self, h: Handle<Function>) -> Result<Work, String> {
        if let Some(n) = self.done.get(&h) {
            return Ok(*n);
        }
        let f = &self.module.functions[h];
        let n = self.block((f, &self.info[h]), &f.body)?;
        self.done.insert(h, n);
        Ok(n)
    }
}
