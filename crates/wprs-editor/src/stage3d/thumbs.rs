use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use wprs_scene::Model;

static THUMBS: Mutex<Option<HashMap<(u64, u16), String>>> = Mutex::new(None);

pub(super) fn thumb(model: &Model, t: u16) -> Option<String> {
    let mut kept = THUMBS.lock().unwrap_or_else(PoisonError::into_inner);
    let kept = kept.get_or_insert_with(HashMap::new);
    if let Some(found) = kept.get(&(model.digest, t)) {
        return Some(found.clone());
    }
    let bytes = &model.textures.get(usize::from(t))?.bytes;
    let url = crate::rigs::url(&image::load_from_memory(bytes).ok()?.thumbnail(64, 64).into_rgba8());
    if kept.len() > 256 {
        kept.clear();
    }
    kept.insert((model.digest, t), url.clone());
    Some(url)
}
