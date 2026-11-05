use crate::{Error, library, media, said, store};
use serde::Deserialize;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use wprs_library::{Item, Kind, qhash};

const MADE: &str = "converting";

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(crate) enum Request {
    ConvertTo { uid: String },
    Converted { uid: String },
    Unconverted { uid: String },
}

fn foreign(uid: &str) -> Error {
    said("android.convert_item", "%1 is not a video of the library", &[&uid])
}

fn made(uid: &str) -> Result<PathBuf, Error> {
    let safe = !uid.is_empty() && uid.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !safe {
        return Err(foreign(uid));
    }
    Ok(store::dir()?.join(MADE).join(format!("{uid}.mp4")))
}

fn video(uid: &str) -> Result<Item, Error> {
    let item = library::with(|lib| Ok(lib.item(uid).ok())).ok().flatten();
    item.filter(|i| i.kind == Kind::Video && i.package.is_none()).ok_or_else(|| foreign(uid))
}

pub(crate) fn request(request: Request) -> Result<Value, Error> {
    match request {
        Request::ConvertTo { uid } => {
            video(&uid)?;
            let path = made(&uid)?;
            fs::create_dir_all(store::dir()?.join(MADE))?;
            let _ = fs::remove_file(&path);
            Ok(json!(path))
        }
        Request::Unconverted { uid } => {
            let _ = fs::remove_file(made(&uid)?);
            Ok(Value::Null)
        }
        Request::Converted { uid } => replace(&uid),
    }
}

fn replace(uid: &str) -> Result<Value, Error> {
    let (old, made) = (video(uid)?, made(uid)?);
    if !made.is_file() || !media::is_video(&made) {
        let _ = fs::remove_file(&made);
        return Err(said("android.convert_failed", "the converted video of %1 cannot be read", &[&uid]));
    }
    let hash: String = qhash(&made)?[..8].iter().map(|b| format!("{b:02x}")).collect();
    let path = store::dir()?.join(store::MEDIA).join(format!("{hash}.mp4"));
    fs::rename(&made, &path)?;
    library::with(|lib| Ok(lib.relink_item(uid, &path, library::now())?))?;
    let media = fs::canonicalize(store::dir()?.join(store::MEDIA))?;
    let (before, after) = (fs::canonicalize(&old.path).ok(), fs::canonicalize(&path).ok());
    if let Some(before) = before.filter(|b| b.starts_with(&media) && Some(b) != after.as_ref()) {
        let _ = fs::remove_file(before);
    }
    for extension in ["jpg", "colors"] {
        let _ = fs::remove_file(store::thumb_file(uid, extension)?);
    }
    store::changed();
    Ok(json!(path))
}
