use super::{View, flight};
use std::time::Duration;

const SETTLED: u32 = 3;
const QUIET: Duration = Duration::from_secs(1);

pub(super) fn settled(v: &mut View) -> Vec<u64> {
    let quiet = v.sent.is_empty() && v.first.is_some_and(|at| at.elapsed() >= QUIET);
    match v.completed >= SETTLED || v.idle || quiet {
        true => std::mem::take(&mut v.pending),
        false => Vec::new(),
    }
}

pub(super) fn later(view: usize) {
    let check = move || {
        std::thread::sleep(QUIET);
        let mut f = flight();
        let Some(v) = f.views.iter_mut().find(|v| v.id == view) else { return };
        let settled = settled(v);
        drop(f);
        settled.into_iter().for_each(wprs_scene::tried);
    };
    let thread = std::thread::Builder::new().name("wprs-trial".into()).spawn(check);
    if let Err(e) = thread {
        tracing::debug!("cannot end the trial of a still view later: {e}");
    }
}
