use crate::args::invalid;
use crate::bus::{self, Event};
use crate::diff::Edit;
use crate::grid::PAGE;
use crate::items::qobject::ItemModel;
use crate::library::Source;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QModelIndex, QString};
use serde::Deserialize;
use serde_json::json;
use std::pin::Pin;
use wprs_client::Error;
use wprs_core::model::ItemSummary;

#[derive(Deserialize)]
struct Page {
    #[serde(default)]
    query: String,
    total: u32,
    items: Vec<ItemSummary>,
}

async fn pages((iface, member, mut key): Source, offset: u32, count: u32) -> Result<Page, Error> {
    let mut all = Page { query: String::new(), total: 0, items: Vec::new() };
    loop {
        let at = offset + all.items.len() as u32;
        let limit = (count - at).min(500);
        let reply = bus::call(iface, member, vec![json!(key), json!(at), json!(limit)]).await?;
        let page = wprs_client::parse::<Page>(&reply).map_err(|e| invalid(e.to_string()))?;
        if !page.query.is_empty() {
            (key, all.query) = (page.query.clone(), page.query);
        }
        let short = page.items.len() < limit as usize;
        all.total = page.total;
        all.items.extend(page.items);
        if short || offset + all.items.len() as u32 >= count {
            return Ok(all);
        }
    }
}

impl ItemModel {
    pub fn fetch(mut self: Pin<&mut Self>, offset: u32) {
        let Some(source) = self.source.clone() else { return };
        let rows = self.grid.rows.len() as u32;
        let count = if offset > 0 || self.fresh { offset + PAGE } else { rows.max(PAGE) };
        let generation = self.generation;
        self.as_mut().set_loading(true);
        let qt = self.qt_thread();
        bus::spawn(async move {
            let page = pages(source, offset, count).await;
            qt.queue(move |mut model| {
                if model.generation != generation {
                    return;
                }
                match page {
                    Ok(page) => model.as_mut().page(offset, page),
                    Err(e) => {
                        model.as_mut().set_loading(false);
                        model.as_mut().set_error(QString::from(&e.message));
                        bus::publish(Event::Error(e.message));
                    }
                }
                model.settle();
            })
            .ok();
        });
    }

    pub fn update(mut self: Pin<&mut Self>) {
        let Some((iface, member, query)) = self.source.clone() else { return };
        let mut uids = std::mem::take(&mut self.as_mut().rust_mut().touched);
        uids.sort();
        uids.dedup();
        let (generation, qt) = (self.generation, self.qt_thread());
        self.as_mut().rust_mut().updating = true;
        bus::spawn(async move {
            let picked =
                pages((iface, member, format!("{query} uid:{}", uids.join(","))), 0, uids.len() as u32);
            let (found, total) = (picked.await, pages((iface, member, query), 0, 0).await);
            qt.queue(move |mut model| {
                model.as_mut().rust_mut().updating = false;
                if model.generation != generation {
                    return;
                }
                match (found, total) {
                    (Ok(found), Ok(total)) => model.as_mut().patched(&uids, found.items, total.total),
                    _ => model.as_mut().rust_mut().dirty = true,
                }
                model.settle();
            })
            .ok();
        });
    }

    fn patched(mut self: Pin<&mut Self>, uids: &[String], found: Vec<ItemSummary>, total: u32) {
        let Some(edits) = self.grid.patch(uids, found) else {
            self.as_mut().rust_mut().dirty = true;
            return;
        };
        let set = edits.into_iter().filter_map(|edit| self.as_mut().edit(edit)).collect();
        self.as_mut().changed(set);
        self.as_mut().rust_mut().grid.total = total;
        self.as_mut().set_total(total as i32);
        self.as_mut().summary();
        self.thumbs();
    }

    fn thumbs(self: Pin<&mut Self>) {
        if let Some((first, last)) = self.range.filter(|(first, last)| last >= first && self.sent.is_empty())
        {
            self.visible(first, last);
        }
    }

    fn page(mut self: Pin<&mut Self>, offset: u32, page: Page) {
        let mut this = self.as_mut().rust_mut();
        (this.grid.size, this.grid.total) = (this.size as u32, page.total);
        if let (0, Some(source)) = (offset, this.source.as_mut())
            && !page.query.is_empty()
        {
            source.2 = page.query;
        }
        if std::mem::take(&mut this.fresh) {
            unsafe { self.as_mut().begin_reset_model() };
            self.as_mut().rust_mut().grid.rows.clear();
            self.as_mut().rust_mut().grid.apply(&Edit::Insert(0, page.items));
            unsafe { self.as_mut().end_reset_model() };
        } else {
            let edits = match offset {
                0 => self.grid.plan(page.items),
                _ if page.items.is_empty() => Vec::new(),
                _ => vec![Edit::Insert((offset as usize).min(self.grid.rows.len()), page.items)],
            };
            let set = edits.into_iter().filter_map(|edit| self.as_mut().edit(edit)).collect();
            self.as_mut().changed(set);
        }
        self.as_mut().set_total(page.total as i32);
        self.as_mut().set_error(QString::default());
        self.as_mut().set_loading(false);
        self.as_mut().summary();
        if offset == 0 {
            self.as_mut().reloaded();
        }
        self.thumbs();
    }

    fn edit(mut self: Pin<&mut Self>, edit: Edit) -> Option<usize> {
        let root = QModelIndex::default();
        unsafe {
            match edit {
                Edit::Remove(first, end) => {
                    self.as_mut().begin_remove_rows(&root, first as i32, end as i32 - 1)
                }
                Edit::Insert(at, ref items) => {
                    self.as_mut().begin_insert_rows(&root, at as i32, (at + items.len()) as i32 - 1)
                }
                Edit::Move(from, to) => {
                    _ = self.as_mut().begin_move_rows(&root, from as i32, from as i32, &root, to as i32)
                }
                Edit::Set(..) => {}
            }
            self.as_mut().rust_mut().grid.apply(&edit);
            match edit {
                Edit::Remove(..) => self.as_mut().end_remove_rows(),
                Edit::Insert(..) => self.as_mut().end_insert_rows(),
                Edit::Move(..) => self.as_mut().end_move_rows(),
                Edit::Set(row, _) => return Some(row),
            }
        }
        None
    }
}
