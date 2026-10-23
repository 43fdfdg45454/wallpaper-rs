use crate::bus;
use crate::grid::{self, ROLES};
use crate::items::qobject::ItemModel;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant, QVector};
use serde_json::json;
use std::collections::HashMap;
use std::pin::Pin;
use wprs_core::model::ItemSummary;

const USER: i32 = 256;

impl ItemModel {
    pub fn visible(mut self: Pin<&mut Self>, first: i32, last: i32) {
        self.as_mut().rust_mut().range = Some((first, last));
        let rows = self.grid.rows.len() as i32;
        if last + 40 >= rows && !self.loading && rows < self.total {
            self.as_mut().fetch(rows as u32);
        }
        let wanted = self.grid.wanted(first.max(0) as usize, last.max(0) as usize);
        if wanted.is_empty() || wanted == self.sent {
            return;
        }
        if self.busy {
            self.as_mut().rust_mut().pending = Some((first, last));
            return;
        }
        let (qt, size) = (self.qt_thread(), self.size);
        let mut this = self.as_mut().rust_mut();
        (this.busy, this.sent) = (true, wanted.clone());
        bus::spawn(async move {
            let reply = bus::call("Library1", "RequestThumbnails", vec![json!(wanted), json!(size)]).await;
            let found =
                reply.ok().and_then(|r| serde_json::from_str::<Vec<HashMap<String, String>>>(&r).ok());
            qt.queue(move |mut model| {
                model.as_mut().rust_mut().busy = false;
                model.as_mut().set_thumbs(found.into_iter().flatten().flatten().collect());
                if let Some((first, last)) = model.as_mut().rust_mut().pending.take() {
                    model.visible(first, last);
                }
            })
            .ok();
        });
    }

    pub fn set_thumbs(mut self: Pin<&mut Self>, thumbs: HashMap<String, String>) {
        let rows =
            thumbs.iter().flat_map(|(uid, path)| self.as_mut().rust_mut().grid.thumb(uid, path)).collect();
        self.changed(rows);
    }

    pub fn changed(mut self: Pin<&mut Self>, rows: Vec<usize>) {
        let (Some(&first), Some(&last)) = (rows.iter().min(), rows.iter().max()) else { return };
        let root = QModelIndex::default();
        let (first, last) = (self.index(first as i32, 0, &root), self.index(last as i32, 0, &root));
        self.as_mut().data_changed(&first, &last, &QVector::default());
    }

    pub fn toggle(mut self: Pin<&mut Self>, row: i32, only: bool) {
        let rows = self.as_mut().rust_mut().grid.toggle(row as usize, only);
        self.as_mut().changed(rows);
        self.summary();
    }

    pub fn summary(mut self: Pin<&mut Self>) {
        let all = |f: fn(&ItemSummary) -> bool| self.grid.selected().all(f);
        let flags = (all(|i| i.removable), all(|i| i.imported), all(|i| i.favorite));
        let count = self.grid.selected().count() as i32;
        self.as_mut().set_selected(count);
        self.as_mut().set_removable(flags.0);
        self.as_mut().set_imported(flags.1);
        self.as_mut().set_favorite(flags.2);
    }

    pub fn selection(&self) -> QString {
        QString::from(&json!(self.grid.selected().map(|i| &i.uid).collect::<Vec<_>>()).to_string())
    }

    pub fn uid(&self, row: i32) -> QString {
        QString::from(self.grid.rows.get(row as usize).map_or("", |r| r.item.uid.as_str()))
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(row) = self.grid.rows.get(index.row() as usize) else { return QVariant::default() };
        let item = &row.item;
        let text = |t: &str| QVariant::from(&QString::from(t));
        match role - USER {
            0 => text(&item.uid),
            1 => text(&item.title),
            2 => text(&grid::kind(item)),
            3 => text(&item.color.map(|c| format!("#{c:06x}")).unwrap_or_default()),
            4 => text(&row.thumb.as_deref().filter(|p| !p.is_empty()).map(bus::url).unwrap_or_default()),
            5 => QVariant::from(&item.favorite),
            6 => QVariant::from(&i32::from(item.rating)),
            7 => text(&grid::info(item)),
            8 => QVariant::from(&item.missing),
            9 => text(item.error.as_deref().unwrap_or_default()),
            10 => text(&item.revoked.map(grid::name).unwrap_or_default()),
            11 => QVariant::from(&row.selected),
            12 => text(&item.tags.join(", ")),
            13 => QVariant::from(&item.package.as_ref().is_some_and(|p| p.asks("mature"))),
            14 => QVariant::from(&item.package.as_ref().is_some_and(|p| p.asks("photosensitive"))),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::default();
        for (i, name) in ROLES.iter().enumerate() {
            roles.insert(USER + i as i32, QByteArray::from(*name));
        }
        roles
    }

    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.grid.rows.len() as i32
    }

    pub fn can_fetch_more(&self, _parent: &QModelIndex) -> bool {
        !self.loading && (self.grid.rows.len() as u32) < self.grid.total
    }

    pub fn fetch_more(self: Pin<&mut Self>, _parent: &QModelIndex) {
        let rows = self.grid.rows.len() as u32;
        self.fetch(rows);
    }
}
