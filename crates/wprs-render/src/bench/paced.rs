use super::flight::{Flight, IN_FLIGHT};
use super::frames::Run;
use super::{WARMUP, spread};
use crate::RenderError;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub(super) fn paced(run: &mut Run<'_>, length: Duration) -> Result<Value, RenderError> {
    let fps = run.bench.fps.max(1);
    let interval = Duration::from_secs(1) / fps;
    let mut flight = Flight::default();
    let begin = Instant::now();
    let (mut next, mut last, mut missed) = (begin, begin, 0u64);
    let (mut gpu, mut cpu, mut eval, mut waits, mut span) = (vec![], vec![], vec![], vec![], vec![]);
    while begin.elapsed() < length {
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        }
        let waited = Instant::now();
        flight.wait(run.r)?;
        let now = Instant::now();
        run.runtime.advance(now.saturating_duration_since(last).as_secs_f64());
        last = now;
        let drawn = run.frame(now, false)?;
        flight.submitted(run.r);
        if flight.sent > WARMUP as u64 {
            gpu.extend(drawn.gpu_ms);
            span.extend(drawn.span_ms);
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
        "frames": flight.sent,
        "paced": {
            "seconds": seconds, "target_fps": fps, "fps": flight.sent as f64 / seconds, "missed_slots": missed,
            "in_flight": IN_FLIGHT, "wait_ms": spread(waits),
        },
        "gpu_ms": spread(gpu), "span_ms": spread(span), "cpu_ms": spread(cpu), "eval_us": spread(eval),
    }))
}
