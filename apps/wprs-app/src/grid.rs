use serde::Serialize;
use serde_json::Value;
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
    pub stale: bool,
}

#[derive(Default)]
pub struct Grid {
    pub rows: Vec<Row>,
    pub total: u32,
    pub size: u32,
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
    pub fn stale(&mut self, uids: &[String]) {
        for row in self.rows.iter_mut().filter(|r| uids.is_empty() || uids.contains(&r.item.uid)) {
            row.stale = row.thumb.is_some();
        }
    }

    pub fn wanted(&self, first: usize, last: usize) -> Vec<String> {
        let rows = self.rows.iter().skip(first).take(last.saturating_sub(first) + 1);
        let wanted = rows.filter(|r| (r.thumb.is_none() || r.stale) && !r.item.missing);
        wanted.map(|r| r.item.uid.clone()).collect()
    }

    pub fn thumb(&mut self, uid: &str, path: &str) -> Vec<usize> {
        let mut changed = Vec::new();
        for (i, row) in self.rows.iter_mut().enumerate().filter(|(_, r)| r.item.uid == uid) {
            row.stale = false;
            if row.thumb.as_deref() != Some(path) {
                row.thumb = Some(path.to_owned());
                changed.push(i);
            }
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

pub fn kind(item: &ItemSummary) -> String {
    match &item.package {
        Some(p) if p.kind == "scene" => p.kind.clone(),
        _ => name(item.kind),
    }
}

pub fn name(value: impl Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(name)) => name,
        _ => String::new(),
    }
}
