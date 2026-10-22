use crate::bus::{self, Event};
use crate::grid::{self, Grid};
use crate::items::qobject::ItemModel;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use std::pin::Pin;

pub type Source = (&'static str, &'static str, String);

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
    pub source: Option<Source>,
    pub generation: u64,
    pub fresh: bool,
    pub dirty: bool,
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
            Event::Changed(topic, ids, _) if topic == "library" || watched == Some(topic.as_str()) => {
                if topic == "library" {
                    self.as_mut().rust_mut().grid.stale(&ids);
                    self.as_mut().rust_mut().sent.clear();
                }
                self.refresh();
            }
            Event::Link(Ok(())) => {
                self.as_mut().rust_mut().sent.clear();
                self.refresh();
            }
            _ => {}
        }
    }

    pub fn search(self: Pin<&mut Self>, text: &QString, kind: &QString, favorite: bool, tag: &QString) {
        let query = grid::compose(&text.to_string(), &kind.to_string(), favorite, &tag.to_string());
        self.open(("Library1", "Search", query));
    }

    pub fn playlist(self: Pin<&mut Self>, uid: &QString) {
        self.open(("Playlists1", "Get", uid.to_string()));
    }

    fn open(mut self: Pin<&mut Self>, source: Source) {
        if self.source.as_ref() == Some(&source) {
            return self.refresh();
        }
        let mut this = self.as_mut().rust_mut();
        (this.fresh, this.dirty, this.generation, this.source) =
            (true, false, this.generation + 1, Some(source));
        self.fetch(0);
    }

    pub fn refresh(mut self: Pin<&mut Self>) {
        match self.loading {
            true => self.as_mut().rust_mut().dirty = true,
            false => self.fetch(0),
        }
    }
}
