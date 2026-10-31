use crate::depth::{self, loaded};
use crate::{Error, library, store};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;
use wprs_core::edit::{Action, Edit, OutputCrop};
use wprs_core::{Fit, UvRect};

pub(crate) const SCREEN: &str = "Android";
const MAX_ZOOM: f32 = 8.0;
const WHOLE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

static DRAFT: Mutex<Option<Draft>> = Mutex::new(None);

struct Draft {
    uid: String,
    path: PathBuf,
    frame: (u32, u32),
    crop: Option<[f32; 4]>,
}

#[derive(Clone)]
pub(crate) struct Framed {
    pub(crate) fit: Fit,
    pub(crate) crop: Option<[f32; 4]>,
    pub(crate) base: Option<[f32; 4]>,
    pub(crate) develop: Option<Edit>,
    pub(crate) map: Option<(PathBuf, SystemTime)>,
}

fn drafts() -> MutexGuard<'static, Option<Draft>> {
    DRAFT.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn draft(uid: &str) -> Option<[f32; 4]> {
    drafts().as_ref().filter(|d| d.uid == uid).and_then(|d| d.crop)
}

pub(crate) fn discard(uid: &str) {
    drafts().take_if(|d| d.uid == uid);
}

pub(crate) fn own(edit: &Edit) -> Option<[f32; 4]> {
    edit.outputs.get(SCREEN).map(|o| o.crop)
}

pub(crate) fn within(base: Option<[f32; 4]>, [x, y, w, h]: [f32; 4]) -> [f32; 4] {
    let [bx, by, bw, bh] = base.unwrap_or(WHOLE);
    [bx + x * bw, by + y * bh, w * bw, h * bh]
}

impl Framed {
    pub(crate) fn shown(&self, uid: &str, preview: bool) -> Option<[f32; 4]> {
        let drafted = preview.then(|| draft(uid)).flatten();
        drafted.map(|own| within(self.base, own)).or(self.crop)
    }
}

pub(crate) fn framed(uid: &str, image: &Path, fit: Fit, preview: bool) -> Framed {
    let edit = loaded(image).unwrap_or_default();
    let (fit, drafted, base) =
        (edit.fit.unwrap_or(fit), preview.then(|| draft(uid)).flatten(), edit.geometry.crop);
    let crop = drafted.or_else(|| own(&edit)).map(|own| within(base, own)).or(base);
    let viewed = drafted.is_some() || matches!(fit, Fit::Fill | Fit::Stretch);
    let mut geometry = edit.geometry;
    geometry.crop = crop.filter(|_| !viewed);
    let (ops, masks, source, levels) = (edit.ops, edit.masks, edit.source, edit.depth);
    let develop = Edit { geometry, ops, masks, source, depth: levels, ..Edit::default() };
    let develop = develop.develops().then_some(develop);
    let map = develop.as_ref().filter(|edit| edit.needs_depth()).and_then(|edit| depth::stamped(image, edit));
    Framed { fit, crop: crop.filter(|_| viewed), base, develop, map }
}

pub(crate) fn placed(
    fit: Fit,
    crop: Option<[f32; 4]>,
    image: (u32, u32),
    screen: (u32, u32),
    pan: f32,
) -> UvRect {
    let [x, y, w, h] = crop.unwrap_or(WHOLE);
    let inner = ((image.0 as f32 * w).round() as u32, (image.1 as f32 * h).round() as u32);
    let mut uv = UvRect::compute(fit, inner, screen);
    if fit == Fit::Fill {
        uv.offset[0] = pan * (1.0 - uv.scale[0]);
    }
    let (offset, scale) = ([x + uv.offset[0] * w, y + uv.offset[1] * h], [uv.scale[0] * w, uv.scale[1] * h]);
    UvRect { offset, scale, repeat: uv.repeat }
}

pub(crate) fn moved(
    crop: Option<[f32; 4]>,
    frame: (u32, u32),
    screen: [u32; 2],
    pan: [f32; 2],
    zoom: f32,
) -> [f32; 4] {
    let aspect = |w: u32, h: u32| w.max(1) as f32 / h.max(1) as f32;
    let ratio = aspect(screen[0], screen[1]) / aspect(frame.0, frame.1);
    let base = if ratio < 1.0 { [ratio, 1.0] } else { [1.0, 1.0 / ratio] };
    let [x, y, w, h] = crop.unwrap_or([(1.0 - base[0]) / 2.0, (1.0 - base[1]) / 2.0, base[0], base[1]]);
    let level = ((base[0] / w.max(1e-3)).min(base[1] / h.max(1e-3)) * zoom).clamp(1.0, MAX_ZOOM);
    let size = base.map(|b| b / level);
    let center = [x + w / 2.0 - pan[0] * w, y + h / 2.0 - pan[1] * h];
    let at = |i: usize| (center[i] - size[i] / 2.0).clamp(0.0, 1.0 - size[i]);
    [at(0), at(1), size[0], size[1]]
}

fn begin(uid: String) -> Result<Draft, Error> {
    let item = library::with(|lib| Ok(lib.item(&uid)?))?;
    let path = crate::pack::media(&item)?.0;
    let edit = loaded(&path)?;
    let frame = edit.geometry.frame((item.meta.width.unwrap_or(1), item.meta.height.unwrap_or(1)));
    Ok(Draft { uid, path, frame, crop: own(&edit) })
}

pub(crate) fn frame(
    uid: String,
    pan: [f32; 2],
    zoom: f32,
    screen: [u32; 2],
    done: bool,
) -> Result<Value, Error> {
    let held = drafts().take_if(|d| d.uid == uid);
    let began = held.is_none();
    let draft = held.map_or_else(|| begin(uid), Ok)?;
    let crop = moved(draft.crop, draft.frame, screen, pan, zoom);
    if done {
        let framing = OutputCrop { connector: SCREEN.into(), crop };
        depth::change(&draft.path, |edit| {
            edit.outputs.insert(SCREEN.into(), framing);
            Ok(())
        })?;
    } else {
        *drafts() = Some(Draft { crop: Some(crop), ..draft });
        if began {
            store::changed();
        }
    }
    Ok(json!(crop))
}

pub(crate) fn unframe(image: &Path, uid: &str) -> Result<Value, Error> {
    discard(uid);
    depth::change(image, |edit| {
        edit.outputs.remove(SCREEN);
        Ok(())
    })?;
    Ok(Value::Null)
}

pub(crate) fn fit(uids: &[String], fit: Option<Fit>) -> Result<Value, Error> {
    let pictures = library::with(|lib| {
        let items = uids.iter().filter_map(|uid| crate::pack::media(&lib.item(uid).ok()?).ok());
        Ok(items.filter(|(_, kind)| *kind == "image").map(|(path, _)| path).collect::<Vec<_>>())
    })?;
    let mode = fit.map_or_else(|| "none".to_owned(), |fit| fit.to_string());
    for image in &pictures {
        let set = Action::Fit { mode: mode.clone() };
        depth::change(image, |edit| edit.act(set).map_err(|e| Error::Said(e.message().clone())))?;
    }
    Ok(json!(pictures.len()))
}
