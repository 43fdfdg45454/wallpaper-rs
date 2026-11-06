use super::Stage;
use crate::extras::{self, Extras};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use wprs_core::model::Input;
use wprs_render::{Pixels, SceneAssets};
use wprs_scene::{Cover, MediaState};

#[derive(Default)]
pub(crate) struct Shown {
    pub(super) granted: bool,
    told: Option<(bool, u64)>,
    covers: Vec<(String, Arc<Pixels>)>,
    placeholder: Option<Option<[[f32; 4]; 5]>>,
}

impl Shown {
    pub(super) fn new(granted: bool) -> Shown {
        Shown { granted, ..Shown::default() }
    }
}

pub(crate) fn allowed(pack: &crate::pack::Pack) -> bool {
    let network = pack.web.as_ref().is_some_and(|w| !w.network.is_empty());
    let id = &pack.id;
    crate::props::allows(id, Input::Media) && (!network || crate::props::allows(id, Input::MediaNetwork))
}

impl Stage {
    pub(crate) fn hears_media(&self) -> bool {
        self.runtime.uses_at(self.quality).media && self.media.granted
    }

    pub(super) fn stocked(&self, extras: &Extras) -> SceneAssets {
        let mut assets = extras.assets(&self.pack);
        assets.images.extend(self.media.covers.iter().cloned());
        assets
    }

    fn placeholder(&mut self) -> Option<[[f32; 4]; 5]> {
        let pack = &self.pack;
        let found = || {
            let mut covers = pack.scene.assets.iter().filter(|(_, a)| a.media == Some(Cover::Current));
            let image = pack.assets.images.get(covers.next()?.0)?.image()?;
            crate::playing::colors_of(&image)
        };
        *self.media.placeholder.get_or_insert_with(found)
    }

    pub(crate) fn hear_media(&mut self) -> bool {
        let allowed = self.hears_media() && crate::playing::access();
        let played = crate::playing::now().filter(|_| allowed);
        let told = (allowed, played.as_ref().map_or(0, |p| p.serial));
        if self.media.told == Some(told) {
            return false;
        }
        self.media.told = Some(told);
        let fallback = || MediaState { enabled: allowed, rate: 1.0, ..MediaState::default() };
        let mut state = played.as_ref().map_or_else(fallback, |p| p.at(Instant::now()));
        if state.colors.is_none() {
            state.colors = self.placeholder();
        }
        let covers: Vec<_> = (self.pack.scene.assets.iter())
            .filter_map(|(id, a)| {
                let p = played.as_ref()?;
                let art = match a.media? {
                    Cover::Current => p.cover.as_ref(),
                    Cover::Previous => p.previous.as_ref(),
                };
                art.map(|art| (id.clone(), art.pixels.clone()))
            })
            .collect();
        let same = |a: &[(String, Arc<Pixels>)], b: &[(String, Arc<Pixels>)]| {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.0 == y.0 && Arc::ptr_eq(&x.1, &y.1))
        };
        let restock = !same(&covers, &self.media.covers);
        if restock {
            self.media.covers = covers;
            self.assets = self.stocked(&self.extras);
            self.restock = true;
        }
        self.runtime.set_media(state) || restock
    }

    pub(crate) fn gather(&mut self) {
        let shown = &self.acting.keeping.shown;
        let mut fresh =
            Extras::gather(&self.pack, (self.runtime.props(), shown), extras::now(), &self.extras);
        if !fresh.same(&self.extras) {
            (self.assets, self.restock, self.touch.2) = (self.stocked(&fresh), true, true);
        }
        let user = |(k, _): &&(String, PathBuf)| !self.pack.videos.iter().any(|(o, _)| o == k);
        let new = |(k, _): &&(String, PathBuf)| !self.extras.videos.iter().any(|(o, _)| o == k);
        let films = fresh.videos.iter().filter(user).filter(new).cloned().collect();
        fresh.lengths = std::mem::take(&mut self.extras.lengths);
        fresh.measure(films);
        self.extras = fresh;
    }
}
