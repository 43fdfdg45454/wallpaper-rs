use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use wprs_core::model::{ItemKind, ItemSummary};

pub const PAGE: u32 = 200;
pub const ROLES: [&str; 13] = [
    "uid", "title", "kind", "color", "thumb", "favorite", "rating", "info", "missing", "broken", "revoked",
    "selected", "tags",
];

pub struct Row {
    pub item: ItemSummary,
    pub thumb: Option<String>,
    pub selected: bool,
}

#[derive(Default)]
pub struct Grid {
    pub rows: Vec<Row>,
    pub total: u32,
    pub size: u32,
}

pub enum Shape {
    Append,
    Grow(usize, usize),
    Shrink(usize, usize),
    Reset,
}

pub fn compose(text: &str, kind: &str, favorite: bool, tag: &str) -> String {
    let quote = |t: &str| {
        if t.contains(char::is_whitespace) { format!("\"{}\"", t.replace('"', "")) } else { t.to_owned() }
    };
    let parts = [
        (!kind.is_empty()).then(|| format!("kind:{kind}")),
        favorite.then(|| "fav:yes".to_owned()),
        (!tag.is_empty()).then(|| format!("tag:{}", quote(tag))),
        Some(text.trim().to_owned()).filter(|t| !t.is_empty()),
    ];
    parts.into_iter().flatten().collect::<Vec<_>>().join(" ")
}

impl Grid {
    pub fn shape(&self, offset: u32, items: &[ItemSummary]) -> Shape {
        let (old, new) = (self.rows.len(), items.len());
        let same = self.rows.iter().zip(items).all(|(row, item)| row.item.uid == item.uid);
        match (offset as usize, same) {
            (o, _) if o == old && o > 0 => Shape::Append,
            (0, true) if new >= old => Shape::Grow(old, new),
            (0, true) => Shape::Shrink(new, old),
            _ => Shape::Reset,
        }
    }

    pub fn store(&mut self, offset: u32, total: u32, items: Vec<ItemSummary>) {
        let keep =
            (offset == 0).then(|| self.rows.drain(..).map(|r| (r.item.uid.clone(), (r.thumb, r.selected))));
        let keep: HashMap<_, _> = keep.into_iter().flatten().collect();
        self.total = total;
        self.rows.extend(items.into_iter().map(|item| {
            let (thumb, selected) = keep.get(&item.uid).cloned().unwrap_or_default();
            let cached = item.thumb.as_ref().filter(|_| self.size == 256).map(|p| p.display().to_string());
            Row { thumb: cached.or(thumb), selected, item }
        }));
    }

    pub fn wanted(&self, first: usize, last: usize) -> Vec<String> {
        let rows = self.rows.iter().skip(first).take(last.saturating_sub(first) + 1);
        rows.filter(|r| r.thumb.is_none() && !r.item.missing).map(|r| r.item.uid.clone()).collect()
    }

    pub fn thumb(&mut self, uid: &str, path: &str) -> Vec<usize> {
        let mut changed = Vec::new();
        for (i, row) in self.rows.iter_mut().enumerate().filter(|(_, r)| r.item.uid == uid) {
            row.thumb = Some(path.to_owned());
            changed.push(i);
        }
        changed
    }

    pub fn toggle(&mut self, row: usize, only: bool) -> Vec<usize> {
        let mut changed = Vec::new();
        for (i, r) in self.rows.iter_mut().enumerate() {
            let selected = match i == row {
                true => !r.selected || only,
                false => r.selected && !only,
            };
            if selected != r.selected {
                r.selected = selected;
                changed.push(i);
            }
        }
        changed
    }

    pub fn selected(&self) -> impl Iterator<Item = &ItemSummary> {
        self.rows.iter().filter(|r| r.selected).map(|r| &r.item)
    }
}

pub fn info(item: &ItemSummary) -> String {
    let size = item.width.zip(item.height).map(|(w, h)| format!("{w}×{h}"));
    let secs = item.duration_ms.map(|ms| ms / 1000).filter(|_| item.kind == ItemKind::Video);
    let time = secs.map(|s| format!("{}:{:02}", s / 60, s % 60));
    [size, time].into_iter().flatten().collect::<Vec<_>>().join(" · ")
}

pub fn name(value: impl Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(name)) => name,
        _ => String::new(),
    }
}
