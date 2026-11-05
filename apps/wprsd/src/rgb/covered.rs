use super::{Ask, LIGHTS};
use crate::compositor::lock;
use std::collections::BTreeSet;
use std::sync::Mutex;
use wprs_core::rgb::Covered;

static COVERED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

pub(crate) fn cover(source: &str, covered: bool) {
    let mut set = lock(&COVERED);
    let changed = if covered { set.insert(source.into()) } else { set.remove(source) };
    drop(set);
    if let Some(l) = LIGHTS.get().filter(|_| changed) {
        l.asks.send(Ask::Covered).ok();
    }
}

pub(crate) fn covered(source: &str) -> bool {
    lock(&COVERED).contains(source)
}

fn mode() -> Option<Covered> {
    LIGHTS.get().map(|l| lock(&l.status).rgb.when_covered)
}

pub(crate) fn any() -> bool {
    !lock(&COVERED).is_empty()
}

pub(crate) fn follows() -> bool {
    let receiving = LIGHTS.get().is_some_and(|l| lock(&l.status).receiving());
    super::enabled() && mode() == Some(Covered::Follow) && receiving
}

pub(crate) fn paused(source: &str) -> bool {
    mode().is_none_or(|m| m == Covered::Pause) && covered(source)
}
