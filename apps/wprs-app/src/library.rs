use crate::args::invalid;
use crate::bus::{self, Event};
use crate::grid::{self, Grid, PAGE, Shape};
use crate::items::qobject::ItemModel;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QModelIndex, QString};
use serde::Deserialize;
use serde_json::json;
use std::pin::Pin;
use wprs_core::model::ItemSummary;

#[derive(Deserialize)]
struct Page {
    #[serde(default)]
    query: String,
    total: u32,
    offset: u32,
    items: Vec<ItemSummary>,
}

#[derive(Default)]
pub struct ItemModelRust {
    pub total: i32,
    pub loading: bool,
    pub error: QString,
    pub size: i32,
    pub selected: i32,
    pub removable: bool,
    pub imported: bool,
    pub favorite: bool,
    pub grid: Grid,
    pub source: Option<(&'static str, &'static str, String)>,
    pub generation: u64,
    pub sent: Vec<String>,
    pub pending: Option<(i32, i32)>,
    pub range: Option<(i32, i32)>,
    pub busy: bool,
}

impl cxx_qt::Initialize for ItemModel {
    fn initialize(mut self: Pin<&mut Self>) {
        self.as_mut().set_size(256);
        bus::listen(self.qt_thread(), ItemModel::event);
    }
}

impl ItemModel {
    fn event(mut self: Pin<&mut Self>, event: Event) {
        let watched = self.source.as_ref().map(|s| bus::topic(s.0, s.1));
        match event {
            Event::Thumb(uid, size, path) if size as i32 == self.size => {
                self.as_mut().set_thumbs([(uid, path)].into())
            }
            Event::Changed(topic, ..) if topic == "library" || watched == Some(topic.as_str()) => {
                self.fetch(0)
            }
            Event::Link(Ok(())) => {
                self.as_mut().rust_mut().sent.clear();
                if !self.loading {
                    self.fetch(0);
                }
            }
            _ => {}
        }
    }

    pub fn search(mut self: Pin<&mut Self>, text: &QString, kind: &QString, favorite: bool, tag: &QString) {
        let query = grid::compose(&text.to_string(), &kind.to_string(), favorite, &tag.to_string());
        self.as_mut().rust_mut().source = Some(("Library1", "Search", query));
        self.fetch(0);
    }

    pub fn playlist(mut self: Pin<&mut Self>, uid: &QString) {
        self.as_mut().rust_mut().source = Some(("Playlists1", "Get", uid.to_string()));
        self.fetch(0);
    }

    pub fn fetch(mut self: Pin<&mut Self>, offset: u32) {
        let Some((iface, member, key)) = self.source.clone() else { return };
        let limit = if offset == 0 { (self.grid.rows.len() as u32).clamp(PAGE, 500) } else { PAGE };
        let generation = self.generation + u64::from(offset == 0);
        self.as_mut().rust_mut().generation = generation;
        self.as_mut().set_loading(true);
        let qt = self.qt_thread();
        bus::spawn(async move {
            let reply = bus::call(iface, member, vec![json!(key), json!(offset), json!(limit)]).await;
            let page =
                reply.and_then(|json| wprs_client::parse::<Page>(&json).map_err(|e| invalid(e.to_string())));
            qt.queue(move |mut model| {
                if model.generation == generation {
                    model.as_mut().set_loading(false);
                    match page {
                        Ok(page) => model.page(page),
                        Err(e) => {
                            model.set_error(QString::from(&e.message));
                            bus::publish(Event::Error(e.message));
                        }
                    }
                }
            })
            .ok();
        });
    }

    fn page(mut self: Pin<&mut Self>, page: Page) {
        let (old, root) = (self.grid.rows.len() as i32, QModelIndex::default());
        let (new, shape) = (page.items.len() as i32, self.grid.shape(page.offset, &page.items));
        if let (0, Some(source)) = (page.offset, self.as_mut().rust_mut().source.as_mut())
            && !page.query.is_empty()
        {
            source.2 = page.query;
        }
        unsafe {
            match shape {
                Shape::Append if new > 0 => self.as_mut().begin_insert_rows(&root, old, old + new - 1),
                Shape::Grow(a, b) if b > a => self.as_mut().begin_insert_rows(&root, a as i32, b as i32 - 1),
                Shape::Shrink(a, b) => self.as_mut().begin_remove_rows(&root, a as i32, b as i32 - 1),
                Shape::Reset => self.as_mut().begin_reset_model(),
                _ => {}
            }
            self.as_mut().rust_mut().grid.store(page.offset, page.total, page.items);
            match shape {
                Shape::Append if new > 0 => self.as_mut().end_insert_rows(),
                Shape::Grow(a, b) if b > a => self.as_mut().end_insert_rows(),
                Shape::Shrink(..) => self.as_mut().end_remove_rows(),
                Shape::Reset => self.as_mut().end_reset_model(),
                _ => {}
            }
        }
        if let Shape::Grow(n, _) | Shape::Shrink(n, _) = shape {
            self.as_mut().changed((0..n).collect());
        }
        self.as_mut().set_total(page.total as i32);
        self.as_mut().set_error(QString::default());
        self.as_mut().summary();
        if let Some((first, last)) = self.range.filter(|(first, last)| last >= first && self.sent.is_empty())
        {
            self.visible(first, last);
        }
    }
}
