use super::{Frame, Shown, WINDOW_NS, log, monotonic, on};
use std::fmt::Write;
use std::time::Duration;

const PASSES: usize = 20;
const SCREENS: [&str; 2] = ["home", "lock"];

pub(crate) fn report() -> String {
    if !on() {
        return "performance recording is off".into();
    }
    let mut text = written(monotonic());
    let (timed, mut each) = wprs_render::profiled_passes(Some(Duration::from_nanos(WINDOW_NS as u64)));
    each.sort_by(|a, b| b.1.total_cmp(&a.1));
    let total: f64 = each.iter().map(|p| p.1).sum();
    match timed {
        0 => text.push_str(
            "passes       not timed in the last 30 s, or the GPU has no timestamps inside encoders\n",
        ),
        n => {
            let _ = writeln!(
                text,
                "passes       {total:.3} ms a frame, from GPU timestamps of {n} frames in 30 s"
            );
            each.iter().take(PASSES).for_each(|(label, ms)| {
                let _ = writeln!(text, "  {ms:8.4} ms  {label}");
            });
        }
    }
    text
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

fn latest(now: i64) -> Option<(usize, Vec<Frame>, Shown, f64)> {
    log(|log| {
        let recent = |s: &super::Screen| {
            s.frames.back().map(|f| f.at).max(s.spans.last(now)).filter(|at| now - at <= WINDOW_NS)
        };
        let tag = (0..2).filter_map(|t| recent(&log.screens[t]).map(|at| (at, t))).max()?.1;
        let screen = &log.screens[tag];
        let frames = screen.frames.iter().filter(|f| now - f.at <= WINDOW_NS).copied().collect();
        Some((tag, frames, screen.shown, screen.spans.seconds(now)))
    })
}

fn rate(count: usize, seconds: f64) -> f64 {
    if seconds > 0.0 { count as f64 / seconds } else { 0.0 }
}

pub(super) fn written(now: i64) -> String {
    let Some((tag, frames, shown, seen)) = latest(now) else {
        return "frames       none of a shown wallpaper in the last 30 s\n".into();
    };
    let mut text = String::new();
    let gaps: Vec<f32> =
        frames.windows(2).filter(|w| !w[1].fresh).map(|w| (w[1].at - w[0].at) as f32 / 1e6).collect();
    let animating = gaps.iter().map(|&g| f64::from(g)).sum::<f64>() / 1e3;
    let seen = seen.max(animating);
    let runs = frames.iter().enumerate().filter(|(i, f)| *i == 0 || f.fresh).count();
    let of = |pick: fn(&Frame) -> f32| spread(frames.iter().map(pick).collect());
    let mean = |pick: fn(&Frame) -> u32| {
        frames.iter().map(|f| pick(f) as f32).sum::<f32>() / frames.len().max(1) as f32
    };
    let _ = writeln!(text, "screen       {}, the one shown last", SCREENS[tag]);
    let _ = writeln!(text, "frames       {} presented in the last 30 s ({runs} runs)", frames.len());
    let _ = writeln!(text, "time         {seen:.1} s shown, {animating:.1} s animating");
    let (per_shown, per_moving) = (rate(frames.len(), seen), rate(gaps.len(), animating));
    let _ =
        writeln!(text, "rate         {per_shown:.1} per second shown, {per_moving:.1} per second animating");
    if frames.is_empty() {
        return text;
    }
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
    let hz = frames.iter().map(|f| f.hz);
    let (low, high) = hz.fold((f32::MAX, 0.0_f32), |(l, h), hz| (l.min(hz), h.max(hz)));
    let _ = writeln!(text, "display      {low:.1} to {high:.1} Hz while drawing");
    let _ = writeln!(text, "interval     {}", spread(gaps));
    let _ = writeln!(text, "callback     {} after the vsync", of(|f| f.late));
    let looped: Vec<f32> = frames.iter().filter_map(|f| f.looped).collect();
    let _ = writeln!(
        text,
        "frame loop   {} (Kotlin, JNI and draw, {} frames)",
        spread(looped.clone()),
        looped.len()
    );
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
    text
}
