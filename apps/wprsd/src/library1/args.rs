use crate::error::{Result, invalid};
use crate::thumbs::SIZES;
use std::path::PathBuf;

pub(super) fn text(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

pub(super) fn strs(list: &[String]) -> Vec<&str> {
    list.iter().map(String::as_str).collect()
}

pub(super) fn thumb_size(size: u32) -> Result<u32> {
    if SIZES.contains(&size) { Ok(size) } else { Err(invalid(format!("size must be one of {SIZES:?}"))) }
}
