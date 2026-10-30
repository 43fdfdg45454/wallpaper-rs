use std::sync::{Mutex, MutexGuard, PoisonError};

const KEPT: usize = 64;

static SLOW: Mutex<Vec<(u64, String)>> = Mutex::new(Vec::new());

fn lock() -> MutexGuard<'static, Vec<(u64, String)>> {
    SLOW.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn charge(key: u64, why: &str) -> bool {
    let mut slow = lock();
    if slow.iter().any(|(k, _)| *k == key) {
        return false;
    }
    slow.push((key, why.to_owned()));
    let old = slow.len().saturating_sub(KEPT);
    slow.drain(..old);
    true
}

pub(crate) fn slow(key: u64) -> Option<String> {
    lock().iter().find(|(k, _)| *k == key).map(|(_, why)| why.clone())
}

pub(crate) fn charged(key: u64) -> bool {
    lock().iter().any(|(k, _)| *k == key)
}

pub(crate) fn pardon() -> usize {
    std::mem::take(&mut *lock()).len()
}
