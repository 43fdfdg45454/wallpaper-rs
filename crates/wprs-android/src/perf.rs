use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Mutex, PoisonError};
use std::time::Instant;
use wprs_scene::{Quality, Tier, Why};

pub(crate) mod report;
pub(crate) mod spans;

const WINDOW_NS: i64 = 30_000_000_000;
const MOST: usize = 8_192;

static ON: AtomicBool = AtomicBool::new(false);
static LOG: Mutex<Option<Log>> = Mutex::new(None);

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Frame {
    pub(crate) at: i64,
    pub(crate) fresh: bool,
    pub(crate) late: f32,
    pub(crate) stage: f32,
    pub(crate) encode: f32,
    pub(crate) present: f32,
    pub(crate) total: f32,
    pub(crate) looped: Option<f32>,
    pub(crate) hz: f32,
    pub(crate) gpu: f32,
    pub(crate) sim: f32,
    pub(crate) passes: u32,
    pub(crate) draws: u32,
    pub(crate) created: u32,
    pub(crate) allocations: u32,
}

#[derive(Debug, Clone, Copy, Default)]
struct Shown {
    level: Option<(Tier, Quality, Why)>,
    render: (u32, u32),
    screen: (u32, u32),
    fps: u32,
    cap: u32,
    present: (&'static str, u32),
}

#[derive(Default)]
struct Screen {
    frames: VecDeque<Frame>,
    shown: Shown,
    split: bool,
    spans: spans::Spans,
}

#[derive(Default)]
struct Log {
    part: Frame,
    staged: Option<Shown>,
    present: Option<(&'static str, u32)>,
    screens: [Screen; 2],
    last: Option<usize>,
}

fn log<T>(with: impl FnOnce(&mut Log) -> T) -> T {
    with(LOG.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert_with(Log::default))
}

pub(crate) fn on() -> bool {
    ON.load(Relaxed)
}

pub(crate) fn record(on: bool) {
    if ON.swap(on, Relaxed) != on {
        log(|log| *log = Log::default());
        wprs_render::reset_profiled_passes();
        if on {
            spans::reopen(monotonic());
        }
    }
}

pub(crate) fn measures(asked: bool, preview: bool, hidden: bool) -> bool {
    asked && !preview && !hidden
}

pub(crate) fn staged(
    (stage, encode): (f32, f32),
    level: (Tier, Quality, Why),
    (render, screen): ((u32, u32), (u32, u32)),
    (fps, cap): (u32, u32),
    stats: wprs_render::SceneStats,
) {
    let (gpu, sim) = (stats.gpu_us as f32 / 1e3, stats.sim_gpu_us as f32 / 1e3);
    log(|log| {
        log.staged = Some(Shown { level: Some(level), render, screen, fps, cap, present: ("", 0) });
        let part = &mut log.part;
        (part.stage, part.encode, part.gpu, part.sim) = (stage, encode, gpu, sim);
        (part.passes, part.draws, part.created) = (stats.passes, stats.draws, stats.created);
    });
}

pub(crate) fn presented(ms: f32, mode: &'static str, latency: u32) {
    log(|log| (log.part.present, log.present) = (ms, Some((mode, latency))));
}

pub(crate) fn enter(measured: bool) -> Option<(Instant, i64)> {
    if !measured {
        return None;
    }
    log(|log| (log.part, log.staged, log.present) = (Frame::default(), None, None));
    crate::counted::watch(true);
    Some((Instant::now(), monotonic()))
}

pub(crate) fn leave(mark: Option<(Instant, i64)>, lock: bool, at: i64, presented: bool) {
    let Some((entered, now)) = mark else { return };
    let allocations = crate::counted::watch(false).0 as u32;
    if presented {
        let times = ((now - at) as f32 / 1e6, entered.elapsed().as_secs_f32() * 1e3);
        keep(usize::from(lock), at, times, (allocations, crate::power::hz()));
    }
}

pub(crate) fn looped(lock: bool, at: i64, ms: f32) {
    log(|log| {
        let last = log.screens[usize::from(lock)].frames.back_mut().filter(|f| f.at == at);
        last.into_iter().for_each(|frame| frame.looped = Some(ms));
    });
}

fn keep(tag: usize, at: i64, (late, total): (f32, f32), (allocations, hz): (u32, f32)) {
    log(|log| {
        let part = std::mem::take(&mut log.part);
        let (staged, present) = (log.staged.take(), log.present.take());
        if log.last.replace(tag) != Some(tag) {
            wprs_render::reset_profiled_passes();
        }
        let screen = &mut log.screens[tag];
        if let Some(staged) = staged {
            screen.shown = Shown { present: screen.shown.present, ..staged };
        }
        screen.shown.present = present.unwrap_or(screen.shown.present);
        let fresh = std::mem::take(&mut screen.split) || screen.frames.is_empty();
        let frame = Frame { at, fresh, late, total, allocations, hz, ..part };
        screen.frames.push_back(frame);
        while screen.frames.len() > MOST || screen.frames.front().is_some_and(|f| at - f.at > WINDOW_NS) {
            screen.frames.pop_front();
        }
    });
}

fn monotonic() -> i64 {
    let mut now = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) };
    now.tv_sec * 1_000_000_000 + now.tv_nsec
}

#[cfg(test)]
mod tests;
