use std::collections::VecDeque;
use std::fmt::Write;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Mutex, PoisonError};
use std::time::Instant;
use wprs_scene::{Quality, Tier, Why};

const WINDOW_NS: i64 = 30_000_000_000;
const MOST: usize = 8_192;
const PASSES: usize = 20;

static ON: AtomicBool = AtomicBool::new(false);
static LOG: Mutex<Option<Log>> = Mutex::new(None);

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Frame {
    pub(crate) at: i64,
    pub(crate) late: f32,
    pub(crate) stage: f32,
    pub(crate) encode: f32,
    pub(crate) present: f32,
    pub(crate) total: f32,
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
struct Log {
    frames: VecDeque<Frame>,
    part: Frame,
    shown: Shown,
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
        wprs_render::profile_passes(on);
    }
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
        let present = log.shown.present;
        log.shown = Shown { level: Some(level), render, screen, fps, cap, present };
        let part = &mut log.part;
        (part.stage, part.encode, part.gpu, part.sim) = (stage, encode, gpu, sim);
        (part.passes, part.draws, part.created) = (stats.passes, stats.draws, stats.created);
    });
}

pub(crate) fn presented(ms: f32, mode: &'static str, latency: u32) {
    log(|log| (log.part.present, log.shown.present) = (ms, (mode, latency)));
}

pub(crate) fn enter(shown: bool) -> Option<(Instant, i64)> {
    let mark = (shown && on()).then(|| (Instant::now(), monotonic()));
    if mark.is_some() {
        crate::counted::watch(true);
    }
    mark
}

pub(crate) fn leave(mark: Option<(Instant, i64)>, at: i64, presented: bool) {
    let Some((entered, now)) = mark else { return };
    let allocations = crate::counted::watch(false).0 as u32;
    if presented {
        keep(at, ((now - at) as f32 / 1e6, entered.elapsed().as_secs_f32() * 1e3), allocations);
    }
}

fn keep(at: i64, (late, total): (f32, f32), allocations: u32) {
    log(|log| {
        let frame = Frame { at, late, total, allocations, ..std::mem::take(&mut log.part) };
        log.frames.push_back(frame);
        while log.frames.len() > MOST || log.frames.front().is_some_and(|f| at - f.at > WINDOW_NS) {
            log.frames.pop_front();
        }
    });
}

fn monotonic() -> i64 {
    let mut now = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) };
    now.tv_sec * 1_000_000_000 + now.tv_nsec
}

fn spread(mut values: Vec<f32>) -> String {
    values.sort_by(f32::total_cmp);
    let at = |q: f32| values.get(((values.len() as f32 - 1.0) * q).round() as usize).copied().unwrap_or(0.0);
    let mean = values.iter().sum::<f32>() / values.len().max(1) as f32;
    format!(
        "mean {mean:.2}, p50 {:.2}, p90 {:.2}, p99 {:.2}, max {:.2} ms",
        at(0.5),
        at(0.9),
        at(0.99),
        at(1.0)
    )
}

pub(crate) fn report() -> String {
    let (frames, shown) = log(|log| (log.frames.iter().copied().collect::<Vec<_>>(), log.shown));
    let mut text = String::new();
    if !on() {
        return "performance recording is off".into();
    }
    let span = frames.last().zip(frames.first()).map_or(0, |(l, f)| l.at - f.at) as f32 / 1e9;
    let fps = if span > 0.0 { (frames.len() - 1) as f32 / span } else { 0.0 };
    let gaps = frames.windows(2).map(|w| (w[1].at - w[0].at) as f32 / 1e6).collect();
    let of = |pick: fn(&Frame) -> f32| spread(frames.iter().map(pick).collect());
    let mean = |pick: fn(&Frame) -> u32| {
        frames.iter().map(|f| pick(f) as f32).sum::<f32>() / frames.len().max(1) as f32
    };
    let _ = writeln!(
        text,
        "frames       {} presented in the last {span:.1} s, {fps:.1} per second",
        frames.len()
    );
    let (mode, latency) = shown.present;
    let _ = writeln!(
        text,
        "target       {} fps, cap {} fps, {mode} with latency {latency}",
        shown.fps, shown.cap
    );
    if let Some((tier, quality, why)) = shown.level {
        let _ = writeln!(text, "level        {tier:?} {quality:?}, {}", why.name());
    }
    let (r, s) = (shown.render, shown.screen);
    let _ = writeln!(text, "size         rendered at {}x{} for a {}x{} screen", r.0, r.1, s.0, s.1);
    let _ = writeln!(text, "interval     {}", spread(gaps));
    let _ = writeln!(text, "callback     {} after the vsync", of(|f| f.late));
    let _ = writeln!(text, "draw cpu     {} in all", of(|f| f.total));
    let _ = writeln!(text, "  stage      {} (scene, scripts, sound)", of(|f| f.stage));
    let _ = writeln!(text, "  encode     {}", of(|f| f.encode));
    let _ = writeln!(text, "  present    {} (acquire, blit, submit, wait)", of(|f| f.present));
    let _ = writeln!(text, "gpu          {} (particles {})", of(|f| f.gpu), of(|f| f.sim));
    let (passes, draws, created) = (mean(|f| f.passes), mean(|f| f.draws), mean(|f| f.created));
    let _ = writeln!(
        text,
        "work         {passes:.1} passes, {draws:.1} draws, {created:.2} GPU resources made a frame"
    );
    let _ = writeln!(text, "allocations  {:.1} a frame on the render thread", mean(|f| f.allocations));
    let (timed, mut each) = wprs_render::profiled_passes();
    each.sort_by(|a, b| b.1.total_cmp(&a.1));
    let total: f32 = each.iter().map(|p| p.1 as f32).sum();
    match timed {
        0 => text
            .push_str("passes       not timed yet, or the GPU has no timestamps inside command encoders\n"),
        n => {
            let _ = writeln!(text, "passes       {total:.3} ms a frame, from GPU timestamps of {n} frames");
            each.iter().take(PASSES).for_each(|(label, ms)| {
                let _ = writeln!(text, "  {ms:8.4} ms  {label}");
            });
        }
    }
    text
}

#[cfg(test)]
mod tests;
