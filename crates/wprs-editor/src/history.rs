use crate::ops::{Op, apply};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_STEPS: usize = 500;
pub const MAX_BYTES: usize = 64 << 20;
pub const MERGE_MS: u64 = 500;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub label: String,
    pub undo: Vec<Op>,
    pub redo: Vec<Op>,
    pub at: u64,
    pub size: usize,
    #[serde(default)]
    pub gesture: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct History {
    pub done: Vec<Step>,
    pub undone: Vec<Step>,
    pub saved: Option<usize>,
    #[serde(skip)]
    pub limits: Option<(usize, usize)>,
}

fn size(ops: &[Op]) -> usize {
    ops.iter().map(Op::size).sum()
}

fn merged(redo: &[Op], ops: &[Op]) -> Option<Vec<Op>> {
    let mut out = redo.to_vec();
    for op in ops {
        let Op::Replace { path, value } = op else { return None };
        let slot = out.iter_mut().find_map(|o| match o {
            Op::Add { path: p, value: v } | Op::Replace { path: p, value: v } if p == path => Some(v),
            _ => None,
        })?;
        *slot = value.clone();
    }
    Some(out)
}

impl History {
    pub fn record(
        &mut self,
        doc: &mut Value,
        label: &str,
        ops: Vec<Op>,
        at: u64,
        gesture: bool,
    ) -> Result<Vec<Op>> {
        let undo = apply(doc, &ops)?;
        if self.saved.is_some_and(|s| s > self.done.len()) {
            self.saved = None;
        }
        self.undone.clear();
        let len = self.done.len();
        let last = self
            .done
            .last_mut()
            .filter(|l| gesture && l.gesture && l.label == label && at.saturating_sub(l.at) < MERGE_MS);
        match last.and_then(|l| merged(&l.redo, &ops).map(|redo| (l, redo))) {
            Some((last, redo)) => {
                (last.size, last.redo, last.at) = (size(&last.undo) + size(&redo), redo, at);
                self.saved = self.saved.filter(|s| *s != len);
            }
            None => {
                let size = size(&undo) + size(&ops);
                self.done.push(Step { label: label.into(), undo, redo: ops.clone(), at, size, gesture });
            }
        }
        self.trim();
        Ok(ops)
    }

    pub fn extend(&mut self, doc: &mut Value, ops: Vec<Op>) -> Result<Vec<Op>> {
        let undo = apply(doc, &ops)?;
        if let Some(last) = self.done.last_mut() {
            last.size += size(&undo) + size(&ops);
            last.undo.splice(0..0, undo);
            last.redo.extend(ops.iter().cloned());
        }
        Ok(ops)
    }

    fn trim(&mut self) {
        let (steps, bytes) = self.limits.unwrap_or((MAX_STEPS, MAX_BYTES));
        let mut total: usize = self.done.iter().chain(&self.undone).map(|s| s.size).sum();
        while self.done.len() > 1 && (self.done.len() > steps || total > bytes) {
            total -= self.done.remove(0).size;
            self.saved = self.saved.and_then(|s| s.checked_sub(1));
        }
    }

    pub fn undo(&mut self, doc: &mut Value) -> Result<Option<(String, Vec<Op>)>> {
        let Some(step) = self.done.pop() else { return Ok(None) };
        if let Err(e) = apply(doc, &step.undo) {
            self.done.push(step);
            return Err(e);
        }
        let done = (step.label.clone(), step.undo.clone());
        self.undone.push(step);
        Ok(Some(done))
    }

    pub fn redo(&mut self, doc: &mut Value) -> Result<Option<(String, Vec<Op>)>> {
        let Some(step) = self.undone.pop() else { return Ok(None) };
        if let Err(e) = apply(doc, &step.redo) {
            self.undone.push(step);
            return Err(e);
        }
        let done = (step.label.clone(), step.redo.clone());
        self.done.push(step);
        Ok(Some(done))
    }

    pub fn mark_saved(&mut self) {
        self.saved = Some(self.done.len());
    }

    pub fn dirty(&self) -> bool {
        self.saved != Some(self.done.len())
    }

    pub fn labels(&self) -> (Vec<&str>, Vec<&str>) {
        let done = self.done.iter().map(|s| s.label.as_str()).collect();
        (done, self.undone.iter().rev().map(|s| s.label.as_str()).collect())
    }
}
