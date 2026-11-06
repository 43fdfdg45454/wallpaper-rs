use gstreamer as gst;
use gstreamer::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

const FIRST_FRAME: Duration = Duration::from_secs(1);
const LOOK_AGAIN: Duration = Duration::from_millis(10);

#[derive(Default)]
pub(crate) struct Wish {
    playing: AtomicBool,
    asked: AtomicU64,
    applied: AtomicU64,
}

impl Wish {
    pub(crate) fn new(playing: bool) -> Wish {
        Wish { playing: AtomicBool::new(playing), ..Wish::default() }
    }

    pub(crate) fn set(&self, playing: bool) {
        self.playing.store(playing, Ordering::Release);
        self.asked.fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn read(&self) -> (u64, bool) {
        let asked = self.asked.load(Ordering::Acquire);
        (asked, self.playing.load(Ordering::Acquire))
    }

    pub(crate) fn paused(&self, playbin: &gst::Element) -> bool {
        let (asked, playing) = self.read();
        let (_, current, pending) = playbin.state(gst::ClockTime::ZERO);
        let settled = current == gst::State::Paused && pending == gst::State::VoidPending;
        !playing && settled && self.applied.load(Ordering::Acquire) == asked
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Change {
    Kept,
    Played,
    Paused,
}

pub(crate) struct Gate {
    seen: Arc<AtomicU64>,
    played: Option<(u64, Instant)>,
}

impl Gate {
    pub(crate) fn new(seen: Arc<AtomicU64>) -> Gate {
        Gate { seen, played: None }
    }

    fn early(&self) -> bool {
        let seen = self.seen.load(Ordering::Relaxed);
        self.played.is_some_and(|(at, when)| at > 0 && seen == at && when.elapsed() < FIRST_FRAME)
    }

    pub(crate) fn wait(&self, wanted: bool) -> Option<gst::ClockTime> {
        (!wanted && self.early()).then(|| gst::ClockTime::from_mseconds(LOOK_AGAIN.as_millis() as u64))
    }

    pub(crate) fn apply(
        &mut self,
        playbin: &gst::Element,
        (asked, playing): (u64, bool),
        wish: &Wish,
    ) -> Result<Option<Change>, gst::StateChangeError> {
        let wanted = if playing { gst::State::Playing } else { gst::State::Paused };
        let (_, current, pending) = playbin.state(gst::ClockTime::ZERO);
        let settled = pending == gst::State::VoidPending && current >= gst::State::Paused;
        if !settled || !playing && current == gst::State::Playing && self.early() {
            return Ok(None);
        }
        let change = match current == wanted {
            true => Change::Kept,
            false => {
                playbin.set_state(wanted)?;
                if playing { Change::Played } else { Change::Paused }
            }
        };
        if change == Change::Played {
            self.played = Some((self.seen.load(Ordering::Relaxed), Instant::now()));
        }
        wish.applied.store(asked, Ordering::Release);
        Ok(Some(change))
    }
}
