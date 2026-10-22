use crate::grid::{Grid, Row};
use std::collections::HashSet;
use wprs_core::model::ItemSummary;

#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    Remove(usize, usize),
    Insert(usize, Vec<ItemSummary>),
    Move(usize, usize),
    Set(usize, ItemSummary),
}

impl Grid {
    pub fn plan(&self, items: Vec<ItemSummary>) -> Vec<Edit> {
        let keep: HashSet<&str> = items.iter().map(|i| i.uid.as_str()).collect();
        let mut rows: Vec<Option<&ItemSummary>> = self.rows.iter().map(|r| Some(&r.item)).collect();
        let (mut edits, mut i, count) = (Vec::new(), rows.len(), items.len());
        while i > 0 {
            let end = i;
            while i > 0 && rows[i - 1].is_some_and(|r| !keep.contains(r.uid.as_str())) {
                i -= 1;
            }
            if i < end {
                rows.drain(i..end);
                edits.push(Edit::Remove(i, end));
            } else {
                i -= 1;
            }
        }
        for (i, item) in items.into_iter().enumerate() {
            match rows[i..].iter().position(|r| r.is_some_and(|r| r.uid == item.uid)) {
                Some(0) => {}
                Some(j) => {
                    let row = rows.remove(i + j);
                    rows.insert(i, row);
                    edits.push(Edit::Move(i + j, i));
                }
                None => {
                    rows.insert(i, None);
                    match edits.last_mut() {
                        Some(Edit::Insert(at, list)) if *at + list.len() == i => list.push(item),
                        _ => edits.push(Edit::Insert(i, vec![item])),
                    }
                    continue;
                }
            }
            if rows[i].is_some_and(|row| *row != item) {
                edits.push(Edit::Set(i, item));
            }
        }
        if rows.len() > count {
            edits.push(Edit::Remove(count, rows.len()));
        }
        edits
    }

    pub fn apply(&mut self, edit: &Edit) {
        let cached = |item: &ItemSummary| {
            item.thumb.as_ref().filter(|_| self.size == 256).map(|p| p.display().to_string())
        };
        match edit {
            Edit::Remove(first, end) => _ = self.rows.drain(first..end),
            Edit::Insert(at, items) => {
                let rows: Vec<Row> = items
                    .iter()
                    .map(|item| Row {
                        thumb: cached(item),
                        selected: false,
                        stale: false,
                        item: item.clone(),
                    })
                    .collect();
                self.rows.splice(at..at, rows);
            }
            Edit::Move(from, to) => {
                let row = self.rows.remove(*from);
                self.rows.insert(*to, row);
            }
            Edit::Set(i, item) => {
                if let Some(thumb) = cached(item) {
                    (self.rows[*i].thumb, self.rows[*i].stale) = (Some(thumb), false);
                }
                self.rows[*i].item = item.clone();
            }
        }
    }
}
