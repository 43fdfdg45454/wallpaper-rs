#[path = "../fixture/mod.rs"]
mod fixture;

use fixture::{DAWN, DAY, DUSK, NOW, Repo, file_url, files, key};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use wprs_format::{
    Anchor, Error, FileFetch, Reason, add, add_repo, browse, install, installed, publish, remove,
    remove_repo, repos, revoke, revoke_key, update_repo, verify,
};

mod keys;
mod trust;

fn mode(path: &std::path::Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}
