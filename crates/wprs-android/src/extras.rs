use crate::chosen::{kind, listed};
use crate::pack::Pack;
use serde_json::Value as Json;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::time::{SystemTime, UNIX_EPOCH};
use wprs_render::{Pixels, SceneAssets};
use wprs_scene::{Control, Runtime, Values};

const EDGE: u32 = 2048;

#[derive(Default)]
pub(crate) struct Extras {
    images: Vec<(String, PathBuf, Arc<Pixels>)>,
    pub(crate) videos: Arc<Vec<(String, PathBuf)>>,
    pub(crate) next: Option<u64>,
    pub(crate) lengths: Vec<mpsc::Receiver<Vec<(String, f32)>>>,
}

pub(crate) fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn chosen(pack: &Pack, id: &str, path: &Path, now: u64, next: &mut Option<u64>) -> Option<PathBuf> {
    let prop = pack.properties.get(id)?;
    let Control::Folder { every, .. } = prop.control else {
        return path.is_file().then(|| path.to_owned());
    };
    let every = u64::from(every.max(1));
    let files: Vec<PathBuf> =
        listed(path).into_iter().filter(|f| kind(f).is_some_and(|k| prop.accepts(k))).collect();
    let turn = (now / every + 1) * every;
    if files.len() > 1 {
        *next = Some(next.map_or(turn, |at| at.min(turn)));
    }
    files.get(prop.folder_turn(now, files.len())?).cloned()
}

impl Extras {
    pub(crate) fn gather(
        pack: &Pack,
        (values, shown): (&Values, &[(String, PathBuf)]),
        now: u64,
        old: &Extras,
    ) -> Extras {
        let (mut images, mut videos, mut next) = (Vec::new(), Vec::new(), None);
        for (key, file) in shown {
            if kind(file) == Some("video") {
                videos.extend((videos.len() < crate::keeping::SHOWN).then(|| (key.clone(), file.clone())));
                continue;
            }
            let kept = old.images.iter().find(|(k, f, _)| k == key && f == file).map(|(.., p)| Arc::clone(p));
            let decoded = kept.or_else(|| wprs_render::decode_image(file, EDGE).ok().map(Pixels::new));
            images.extend(decoded.map(|pixels| (key.clone(), file.clone(), pixels)));
        }
        for (id, prop) in &pack.properties {
            let Some(path) = values.get(id).and_then(Json::as_str).filter(|v| v.starts_with('/')) else {
                continue;
            };
            let Some(file) = chosen(pack, id, Path::new(path), now, &mut next) else { continue };
            let keys = match &prop.control {
                Control::File { default_asset: Some(asset), .. } => vec![asset.clone(), id.clone()],
                _ => vec![id.clone()],
            };
            if kind(&file) == Some("video") {
                videos.extend(keys.into_iter().map(|key| (key, file.clone())));
                videos.extend(crate::pack::lanes(pack, id, &file));
                continue;
            }
            let kept = old.images.iter().find(|(_, f, _)| *f == file).map(|(.., pixels)| Arc::clone(pixels));
            let pixels =
                match kept.map_or_else(|| wprs_render::decode_image(&file, EDGE).map(Pixels::new), Ok) {
                    Ok(pixels) => pixels,
                    Err(e) => {
                        log::warn!("cannot use the file chosen for a scene property: {e}");
                        continue;
                    }
                };
            images.extend(keys.into_iter().map(|key| (key, file.clone(), Arc::clone(&pixels))));
        }
        let replaced =
            |id: &String| images.iter().any(|(key, ..)| key == id) || videos.iter().any(|(key, _)| key == id);
        let own: Vec<_> = pack.videos.iter().filter(|(id, _)| !replaced(id)).cloned().collect();
        let videos = Arc::new(own.into_iter().chain(videos).collect());
        Extras { images, videos, next, lengths: Vec::new() }
    }

    pub(crate) fn measure(&mut self, films: Vec<(String, PathBuf)>) {
        if films.is_empty() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let measured = move || {
            let found =
                films.into_iter().filter_map(|(key, path)| Some((key, wprs_scene::film_file(&path)?)));
            drop(tx.send(found.collect()));
        };
        if std::thread::Builder::new().name("wprs-lengths".into()).spawn(measured).is_ok() {
            self.lengths.push(rx);
        }
    }

    pub(crate) fn timed(&mut self, runtime: &mut Runtime) -> bool {
        let before = self.lengths.len();
        self.lengths.retain(|rx| match rx.try_recv() {
            Ok(found) => {
                found.iter().for_each(|(key, seconds)| runtime.set_length(key, *seconds));
                false
            }
            Err(mpsc::TryRecvError::Empty) => true,
            Err(mpsc::TryRecvError::Disconnected) => false,
        });
        self.lengths.len() != before
    }

    pub(crate) fn same(&self, other: &Extras) -> bool {
        let shown = |e: &Extras| {
            e.images.iter().map(|(key, file, _)| (key.clone(), file.clone())).collect::<Vec<_>>()
        };
        shown(self) == shown(other) && self.videos == other.videos
    }

    pub(crate) fn assets(&self, pack: &Pack) -> SceneAssets {
        let mut assets = pack.assets.clone();
        assets.images.extend(self.images.iter().map(|(key, _, pixels)| (key.clone(), Arc::clone(pixels))));
        assets
    }
}
