use crate::helper::Request;
use crate::{MAX_ARGUMENTS, MAX_BRACKETS, MAX_OPERANDS};
use crate::{MAX_BUILD, MAX_INLINED, MAX_LOOP, MAX_MEMORY, MAX_OPS, MAX_SHADER_NODES, MAX_WGSL, MAX_WORK};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

const SALT: &str = "wprs-scene checked-effects 1";
const FILE: &str = "checked-effects";
const REMEMBERED: usize = 4096;

type Refusal = Option<String>;
type Known = (BTreeMap<String, (Refusal, u64)>, u64);

static KNOWN: Mutex<Known> = Mutex::new((BTreeMap::new(), 0));
static PASSED: Mutex<BTreeSet<u64>> = Mutex::new(BTreeSet::new());

pub(crate) fn hash(id: &str, json: &str, wgsl: &str) -> String {
    let limits = [MAX_WGSL, MAX_SHADER_NODES, MAX_ARGUMENTS, MAX_BRACKETS, MAX_OPERANDS].map(|n| n as u64);
    let limits = limits.into_iter().chain([MAX_LOOP, MAX_WORK, MAX_OPS, MAX_INLINED, MAX_MEMORY, MAX_BUILD]);
    let mut digest =
        Sha256::new().chain_update(SALT).chain_update([0]).chain_update(env!("CARGO_PKG_VERSION"));
    limits.for_each(|n| digest.update(n.to_le_bytes()));
    for part in [id, json, wgsl] {
        digest.update([0]);
        digest.update(part);
    }
    digest.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn file() -> Option<PathBuf> {
    Some(crate::ledger::unread().path.as_deref()?.with_file_name(FILE))
}

fn known() -> std::sync::MutexGuard<'static, Known> {
    KNOWN.lock().unwrap_or_else(PoisonError::into_inner)
}

fn recall(hash: &str) -> Option<Refusal> {
    let (map, tick) = &mut *known();
    *tick += 1;
    map.get_mut(hash).map(|(refusal, used)| (*used = *tick, refusal.clone()).1)
}

fn remember(hash: &str, refusal: &Refusal) {
    let (map, tick) = &mut *known();
    if map.len() >= REMEMBERED && !map.contains_key(hash) {
        let oldest = map.iter().min_by_key(|(_, (_, used))| *used).map(|(h, _)| h.clone());
        oldest.map(|h| map.remove(&h));
    }
    *tick += 1;
    map.insert(hash.to_owned(), (refusal.clone(), *tick));
}

pub(crate) fn vetted(id: &str, json: &str, wgsl: &str) -> Result<(), String> {
    let Some(checker) = crate::outside::checker() else { return Ok(()) };
    let hash = hash(id, json, wgsl);
    let found = recall(&hash);
    let file = file();
    let found = found.or_else(|| file.as_deref().and_then(|f| crate::kept::read(f, &hash)));
    let refusal = match found {
        Some(refusal) => refusal,
        None => {
            let request = Request { id: id.into(), json: json.into(), wgsl: wgsl.into() };
            let refusal = crate::outside::ask(&checker, &request, &hash)?;
            if let Some(file) = &file {
                drop(crate::kept::write(file, &hash, &refusal));
            }
            refusal
        }
    };
    remember(&hash, &refusal);
    refusal.map_or(Ok(()), Err)
}

pub(crate) fn pass(key: u64) {
    PASSED.lock().unwrap_or_else(PoisonError::into_inner).insert(key);
}

pub(crate) fn passed(key: u64) -> bool {
    crate::outside::checker().is_none()
        || PASSED.lock().unwrap_or_else(PoisonError::into_inner).contains(&key)
}
