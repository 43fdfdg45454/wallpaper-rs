use super::SceneState;
use crate::media::{Cover, Frame};
use std::sync::Arc;
use std::time::Instant;
use wprs_render::Pixels;

pub(in crate::compositor) struct Shown {
    serial: u64,
    serials: [u64; 2],
    pub(super) covers: Vec<(String, Arc<Pixels>)>,
    placeholder: Option<Option<[[f32; 4]; 5]>>,
    pub(super) told: f64,
    pub(super) hears: Option<bool>,
}

impl Default for Shown {
    fn default() -> Shown {
        Shown {
            serial: u64::MAX,
            serials: [0; 2],
            covers: Vec::new(),
            placeholder: None,
            told: -1.0,
            hears: None,
        }
    }
}

fn serial(cover: &Option<Arc<Cover>>) -> u64 {
    cover.as_ref().map_or(0, |c| c.serial)
}

impl SceneState {
    pub(super) fn hear_media(&mut self, frame: Option<Arc<Frame>>) -> bool {
        let serial = frame.as_ref().map_or(0, |f| f.serial);
        if std::mem::replace(&mut self.media.serial, serial) == serial {
            return false;
        }
        let mut state = frame.as_ref().map(|f| f.at(Instant::now())).unwrap_or_default();
        if state.colors.is_none() {
            state.colors = self.placeholder_colors();
        }
        let (cover, previous) = frame.map_or((None, None), |f| (f.cover.clone(), f.previous.clone()));
        let restocked = self.show_covers(cover, previous);
        self.runtime.set_media(state) || restocked
    }

    fn placeholder_colors(&mut self) -> Option<[[f32; 4]; 5]> {
        let pack = &self.spec.pack;
        let found = || {
            let mut covers = pack.scene.assets.iter().filter(|(_, a)| a.media.is_some());
            let id = covers.find(|(_, a)| a.media == Some(wprs_scene::Cover::Current)).map(|(id, _)| id);
            let image = pack.assets.images.get(id?)?.image()?;
            crate::media::covers::colors(&image)
        };
        *self.media.placeholder.get_or_insert_with(found)
    }

    fn show_covers(&mut self, cover: Option<Arc<Cover>>, previous: Option<Arc<Cover>>) -> bool {
        let serials = [serial(&cover), serial(&previous)];
        if std::mem::replace(&mut self.media.serials, serials) == serials {
            return false;
        }
        let assets = self.spec.pack.scene.assets.iter();
        let shown = assets.filter_map(|(id, a)| {
            let picked = match a.media? {
                wprs_scene::Cover::Current => &cover,
                wprs_scene::Cover::Previous => &previous,
            };
            picked.as_ref().map(|c| (id.clone(), c.pixels.clone()))
        });
        let covers: Vec<_> = shown.collect();
        let same = |a: &[(String, Arc<Pixels>)], b: &[(String, Arc<Pixels>)]| {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.0 == y.0 && Arc::ptr_eq(&x.1, &y.1))
        };
        if same(&covers, &self.media.covers) {
            return false;
        }
        self.media.covers = covers;
        self.restock = true;
        true
    }
}
