#[derive(Clone, Copy, Default)]
pub(crate) struct Work {
    pub(crate) ops: u64,
    pub(crate) reads: u64,
    pub(crate) size: u64,
}

impl Work {
    pub(crate) fn then(self, o: Work) -> Work {
        let add = u64::saturating_add;
        Work { ops: add(self.ops, o.ops), reads: add(self.reads, o.reads), size: add(self.size, o.size) }
    }

    pub(crate) fn or(self, o: Work) -> Work {
        Work {
            ops: self.ops.max(o.ops),
            reads: self.reads.max(o.reads),
            size: self.size.saturating_add(o.size),
        }
    }

    pub(crate) fn times(self, n: u64) -> Work {
        Work { ops: self.ops.saturating_mul(n), reads: self.reads.saturating_mul(n), size: self.size }
    }

    pub(crate) fn step(ops: u64) -> Work {
        Work { ops, reads: 0, size: ops }
    }
}
