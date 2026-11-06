use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use wprs_format::stall;

pub(crate) fn changed(state: wprs_format::Result<Value>) -> anyhow::Result<Value> {
    let mut state = state?;
    state["changed"] = json!(true);
    Ok(state)
}

fn now() -> anyhow::Result<i64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs().try_into()?)
}

pub fn added(dir: &Path, files: &[PathBuf], unlisted: bool) -> anyhow::Result<Value> {
    Ok(stall::added(dir, files, unlisted, now()?)?)
}

fn signer(key: &Path, password: Option<String>) -> anyhow::Result<wprs_format::Signer> {
    Ok(wprs_format::Signer::new(&std::fs::read_to_string(key)?, password)?)
}

pub fn published(dir: &Path, key: &Path, password: Option<String>, days: u32) -> anyhow::Result<Value> {
    Ok(stall::published(dir, &signer(key, password)?, days, now()?)?)
}

fn done(mut state: Value, what: &str) -> Value {
    state["done"] = json!(what);
    state
}

pub fn rooted(dir: &Path, key: &Path, password: Option<String>) -> anyhow::Result<Value> {
    Ok(done(stall::root_signed(dir, &signer(key, password)?)?, "signed"))
}

pub fn chartered(dir: &Path, charter: &str, fresh: bool) -> anyhow::Result<Value> {
    let charter = serde_json::from_str::<wprs_format::Charter>(charter)?;
    Ok(match fresh {
        true => done(stall::made(dir, charter, now()?)?, "created"),
        false => done(stall::rotated(dir, charter, now()?)?, "rotated"),
    })
}
