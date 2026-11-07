use crate::compositor::Deck;
use crate::daemon::Daemon;
use crate::error::failed;
use image::{GrayImage, RgbImage};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use wprs_core::Fill;
use wprs_core::edit::{Depth, Edit, Geometry};
use wprs_core::model::JobKind;
use wprs_depth::{Device, Inpainter, Layers, Places, Settings};

const THREADS: usize = 4;

#[derive(Default)]
pub(crate) struct Memory {
    sets: HashMap<PathBuf, Weak<Layers>>,
    turned: HashMap<(PathBuf, String), Weak<Layers>>,
    failed: HashSet<PathBuf>,
    me: Weak<Daemon>,
}

pub(crate) fn start(daemon: &Arc<Daemon>) {
    daemon.state().decks.me = Arc::downgrade(daemon);
}

fn build(
    places: &Places,
    (file, sha): (&Path, &str),
    (depth, set): (&Depth, &Settings),
    ctl: &crate::jobs::Ctl,
) -> anyhow::Result<PathBuf> {
    let stop = ctl.flag();
    let threads = wprs_depth::threads().min(THREADS);
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                wprs_depth::background();
                let mut loaded = (set.fill == Fill::Lama)
                    .then(|| Inpainter::open(places, None, threads, Device::Auto))
                    .transpose()?;
                let mut paint = |image: &RgbImage, mask: &GrayImage| match loaded.as_mut() {
                    Some(l) => l.run(image, mask),
                    None => anyhow::bail!("LaMa is not loaded"),
                };
                let inpaint: Option<&mut wprs_depth::Inpaint<'_>> =
                    (set.fill == Fill::Lama).then_some(&mut paint);
                places.build_layers(
                    (file, sha),
                    (depth, set),
                    inpaint,
                    &mut |done| ctl.progress(done, 100),
                    &stop,
                )
            })
            .join()
            .unwrap_or_else(|_| anyhow::bail!("the layer builder stopped"))
    })
}

impl Daemon {
    pub(crate) fn deck_for(&self, file: &Path, sha: &str, edit: &Edit, places: &Places) -> Option<Deck> {
        let moving = edit.moving()?;
        let config = self.state().config.parallax.clone();
        if edit.operates() || config.fill == Fill::Off {
            return None;
        }
        let depth = edit.depth.clone().unwrap_or_default();
        let set = Settings {
            fill: places.fill(config.fill),
            count: usize::from(config.layers),
            shift: moving.strength,
        };
        let path = places.layer_set(file, sha, &depth, &set)?;
        let known = self.state().decks.sets.get(&path).and_then(Weak::upgrade);
        let found = match known {
            Some(layers) => Some(layers),
            None => match wprs_depth::load(&path) {
                Ok(found) => found.map(Arc::new),
                Err(e) => {
                    tracing::warn!("cannot use the depth layers of a picture");
                    tracing::debug!("{e:#}");
                    places.discard(&path);
                    None
                }
            },
        };
        let Some(layers) = found else {
            self.build_deck(file, sha, (depth, set), path);
            return None;
        };
        {
            let mut st = self.state();
            st.decks.sets.retain(|_, weak| weak.strong_count() > 0);
            st.decks.sets.insert(path.clone(), Arc::downgrade(&layers));
        }
        Some(Deck { layers: self.turned(&path, layers, &edit.geometry), file: path })
    }

    fn turned(&self, path: &Path, layers: Arc<Layers>, geometry: &Geometry) -> Arc<Layers> {
        if geometry.is_identity() {
            return layers;
        }
        let key = (path.to_owned(), serde_json::to_string(geometry).unwrap_or_default());
        if let Some(known) = self.state().decks.turned.get(&key).and_then(Weak::upgrade) {
            return known;
        }
        let made = Arc::new(wprs_depth::turned(&layers, geometry));
        let mut st = self.state();
        st.decks.turned.retain(|_, weak| weak.strong_count() > 0);
        st.decks.turned.insert(key, Arc::downgrade(&made));
        made
    }

    fn build_deck(&self, file: &Path, sha: &str, (depth, set): (Depth, Settings), path: PathBuf) {
        if self.state().decks.failed.contains(&path) {
            return;
        }
        let (file, sha) = (file.to_owned(), sha.to_owned());
        let Some(daemon) = self.state().decks.me.upgrade() else { return };
        let target = file.to_string_lossy().into_owned();
        daemon.start_job(JobKind::Layers, &target, move |d, ctl| {
            let places = Places { data: d.store.data.clone(), cache: d.store.cache.clone() };
            let built = build(&places, (&file, &sha), (&depth, &set), ctl);
            if let Err(e) = &built {
                if ctl.check().is_err() {
                    return Err(crate::error::Error::Cancelled);
                }
                tracing::warn!("cannot build the depth layers of a picture");
                tracing::debug!("{e:#}");
                d.state().decks.failed.insert(path);
            }
            let made = built.map_err(|e| failed(format!("{e:#}")))?;
            d.configure_all();
            Ok(json!({"file": made, "fill": set.fill.as_str(), "layers": set.count}))
        });
    }
}
