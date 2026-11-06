use super::frames::Run;
use super::{WARMUP, spread};
use crate::RenderError;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const IN_FLIGHT: u64 = 2;
const NAP: Duration = Duration::from_micros(200);

pub(super) fn paced(run: &mut Run<'_>, length: Duration) -> Result<Value, RenderError> {
    let fps = run.bench.fps.max(1);
    let interval = Duration::from_secs(1) / fps;
    let done = Arc::new(AtomicU64::new(0));
    let begin = Instant::now();
    let (mut next, mut last, mut sent, mut missed) = (begin, begin, 0u64, 0u64);
    let (mut gpu, mut cpu, mut eval, mut waits) = (vec![], vec![], vec![], vec![]);
    while begin.elapsed() < length {
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        }
        let waited = Instant::now();
        while sent >= IN_FLIGHT && done.load(Ordering::Acquire) <= sent - IN_FLIGHT {
            run.r.polled(wgpu::PollType::Poll)?;
            std::thread::sleep(NAP);
        }
        let now = Instant::now();
        run.runtime.advance(now.saturating_duration_since(last).as_secs_f64());
        last = now;
        let drawn = run.frame(now, false)?;
        sent += 1;
        let (flag, id) = (Arc::clone(&done), sent);
        run.r.queue.on_submitted_work_done(move || {
            flag.fetch_max(id, Ordering::Release);
        });
        if sent > WARMUP as u64 {
            gpu.extend(drawn.gpu_ms);
            eval.push(drawn.eval_us);
            cpu.push(now.elapsed().as_secs_f64() * 1e3);
            waits.push(now.saturating_duration_since(waited).as_secs_f64() * 1e3);
        }
        next += interval;
        let now = Instant::now();
        if now > next {
            let behind = (now - next).as_nanos() / interval.as_nanos() + 1;
            missed += behind as u64;
            next += interval * behind as u32;
        }
    }
    run.r.finish()?;
    let seconds = begin.elapsed().as_secs_f64();
    Ok(json!({
        "frames": sent,
        "paced": {
            "seconds": seconds, "target_fps": fps, "fps": sent as f64 / seconds, "missed_slots": missed,
            "in_flight": IN_FLIGHT, "wait_ms": spread(waits),
        },
        "gpu_ms": spread(gpu), "cpu_ms": spread(cpu), "eval_us": spread(eval),
    }))
}
