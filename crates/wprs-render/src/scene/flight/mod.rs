mod sent;
mod settle;

pub(crate) use sent::{done, idle, sending, sent};
use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

const STALE: Duration = Duration::from_secs(20);

struct Sent {
    serial: u64,
    index: Option<wgpu::SubmissionIndex>,
    keys: Vec<u64>,
    at: Instant,
}

struct View {
    id: usize,
    sent: VecDeque<Sent>,
    spare: Vec<Vec<u64>>,
    serial: u64,
    pending: Vec<u64>,
    completed: u32,
    first: Option<Instant>,
    idle: bool,
}

struct Flight {
    views: Vec<View>,
    building: Vec<u64>,
}

static FLIGHT: Mutex<Flight> = Mutex::new(Flight { views: Vec::new(), building: Vec::new() });

fn flight() -> MutexGuard<'static, Flight> {
    FLIGHT.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Flight {
    fn view(&mut self, id: usize) -> &mut View {
        match self.views.iter().position(|v| v.id == id) {
            Some(at) => &mut self.views[at],
            None => {
                let (sent, spare, pending) = Default::default();
                let view =
                    View { id, sent, spare, serial: 0, pending, completed: 0, first: None, idle: false };
                self.views.push(view);
                self.views.last_mut().expect("pushed above")
            }
        }
    }
}

pub fn drawn_effects() -> Vec<u64> {
    let f = flight();
    let sent = f.views.iter().flat_map(|v| &v.sent).filter(|s| s.at.elapsed() < STALE);
    let mut all: Vec<u64> = sent.flat_map(|s| &s.keys).chain(&f.building).copied().collect();
    drop(f);
    all.sort_unstable();
    all.dedup();
    all.retain(|k| !super::clock::charged(*k));
    all
}

pub(crate) fn hanging() -> bool {
    let f = flight();
    let fresh = |s: &Sent| s.at.elapsed() < STALE && !s.keys.is_empty();
    f.views.iter().any(|v| !v.pending.is_empty() || v.sent.iter().any(fresh))
}

pub(crate) fn building(key: u64, on: bool) {
    let mut f = flight();
    f.building.retain(|k| *k != key);
    if on {
        f.building.push(key);
    }
}

pub(crate) fn built(key: u64) -> bool {
    !flight().building.contains(&key)
}

pub(crate) fn pending(view: usize, key: u64) {
    flight().view(view).pending.push(key);
}

pub(crate) fn trial(view: usize) -> bool {
    flight().views.iter().any(|v| v.id == view && !v.pending.is_empty())
}

pub(crate) fn flying(view: usize) -> u32 {
    flight().views.iter().find(|v| v.id == view).map_or(0, |v| v.sent.len() as u32)
}

pub(crate) fn forget(view: usize) {
    let mut f = flight();
    let gone: Vec<u64> = f.views.extract_if(.., |v| v.id == view).flat_map(|v| v.pending).collect();
    drop(f);
    gone.into_iter().for_each(wprs_scene::tried);
}
