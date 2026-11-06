use super::{Bench, Level, WARMUP, save_frame, spread};
use crate::{RenderError, Renderer, SceneAssets, SceneView};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use wprs_scene::{Budget, Governor, Quality, Runtime, Spare, Tier, Wake, Why};

pub(super) struct Run<'a> {
    pub(super) r: &'a Renderer,
    pub(super) runtime: Runtime,
    pub(super) view: SceneView,
    pub(super) bench: &'a Bench,
    pub(super) quality: Quality,
    pub(super) inner: (u32, u32),
    pub(super) levels: Vec<(f64, Quality)>,
    pub(super) load_ms: f64,
    pub(super) start: Instant,
    assets: Option<SceneAssets>,
    others: Vec<SceneView>,
    spare: Spare,
    wake: Wake,
    governor: Option<(Governor, Quality)>,
    samples: u64,
}

pub(super) struct Drawn {
    pub(super) eval_us: f64,
    pub(super) gpu_ms: Option<f64>,
}

impl<'a> Run<'a> {
    pub(super) fn new(
        r: &'a Renderer,
        mut runtime: Runtime,
        assets: SceneAssets,
        bench: &'a Bench,
        (tier, quality): (Tier, Quality),
    ) -> Result<Run<'a>, RenderError> {
        let governor = matches!(bench.level, Level::Timed(_)).then(|| {
            let budget = Budget::new(tier, true, bench.fps.max(1));
            let (ceiling, mut governor) = (budget.reach().0, Governor::timed());
            governor.begin(ceiling, budget.auto(&runtime).0);
            (governor, ceiling)
        });
        let quality = governor.as_ref().map_or(quality, |(g, ceiling)| g.level((*ceiling, Why::Full)).0);
        runtime.set_quality(quality);
        let start = Instant::now();
        let mut others = Vec::new();
        for _ in 1..bench.screens.clamp(1, 16) {
            others.push(r.scene_view(&runtime.scene, &assets, runtime.props())?);
        }
        let view = r.scene_view(&runtime.scene, &assets, runtime.props())?;
        let inner = view.inner(r, bench.size, quality);
        let load_ms = start.elapsed().as_secs_f64() * 1e3;
        let assets = governor.is_some().then_some(assets);
        let (spare, wake, samples, levels) = (Spare::default(), Wake::Frame, 0, vec![(0.0, quality)]);
        Ok(Run {
            r,
            runtime,
            view,
            bench,
            quality,
            inner,
            levels,
            load_ms,
            start,
            assets,
            others,
            spare,
            wake,
            governor,
            samples,
        })
    }

    fn steer(&mut self, at: Instant) -> Result<(), RenderError> {
        let Some((governor, ceiling)) = &self.governor else { return Ok(()) };
        let level = governor.level((*ceiling, Why::Full)).0;
        if level == self.quality {
            return Ok(());
        }
        self.quality = level;
        self.levels.push((at.saturating_duration_since(self.start).as_secs_f64(), level));
        if self.runtime.set_quality(level)
            && let Some(assets) = &self.assets
        {
            let scene = (&*self.runtime.scene, self.runtime.props());
            for view in std::iter::once(&mut self.view).chain(&mut self.others) {
                self.r.refresh_scene(view, scene, assets)?;
            }
        }
        Ok(())
    }

    pub(super) fn frame(&mut self, at: Instant, last: bool) -> Result<Drawn, RenderError> {
        self.steer(at)?;
        let (r, quality) = (self.r, self.quality);
        self.inner = self.view.inner(r, self.bench.size, quality);
        let started = Instant::now();
        let frame =
            self.runtime.frame_in(std::mem::take(&mut self.spare), self.view.view(self.inner), quality);
        let eval_us = started.elapsed().as_secs_f64() * 1e6;
        for other in &mut self.others {
            r.render_scene(other, &self.runtime, &frame, self.inner, quality, &[])?;
        }
        let image = r.render_scene(&mut self.view, &self.runtime, &frame, self.inner, quality, &[])?;
        if let Some(file) = self.bench.shot.as_deref().filter(|_| last) {
            save_frame(r, image, self.bench.size, file)?;
        }
        self.wake = frame.wake;
        self.spare = frame.spare();
        let (samples, gpu_ms) = self.view.gpu_time();
        let fresh = std::mem::replace(&mut self.samples, samples) != samples;
        if let Some((governor, ceiling)) = self.governor.as_mut().filter(|_| fresh) {
            let (runtime, fps) = (&self.runtime, self.bench.fps.max(1) as f32);
            let share = match self.bench.level {
                Level::Timed(share) => share,
                _ => 1.0,
            };
            governor.feed(at, gpu_ms as f32, |q| (1000.0 / fps * share, runtime.cost(q)), *ceiling, true);
        }
        Ok(Drawn { eval_us, gpu_ms: fresh.then_some(gpu_ms) })
    }

    pub(super) fn unpaced(&mut self) -> Result<Value, RenderError> {
        let frames = self.bench.frames.max(1);
        let step = Duration::from_secs(1) / self.bench.fps.max(1);
        let (mut eval, mut render, mut gpu, mut sim, mut sim_gpu) = (vec![], vec![], vec![], vec![], vec![]);
        let (mut needed, mut created, mut steps, mut compiled) = (0, 0, 0, 0);
        let (mut since, mut at) = (Duration::ZERO, self.start);
        let (mut flight, mut timed) = (super::flight::Flight::default(), Instant::now());
        for i in 0..frames {
            timed = if i == WARMUP.min(frames - 1) { Instant::now() } else { timed };
            if i > 0 {
                self.runtime.advance(step.as_secs_f64());
                (since, at) = (since + step, at + step);
            }
            let due = match self.wake {
                Wake::Frame => true,
                Wake::At(s) => since.as_secs_f64() >= s,
                Wake::Idle => false,
            };
            needed += usize::from(due);
            since = if due { Duration::ZERO } else { since };
            let started = Instant::now();
            if self.bench.overlap {
                flight.wait(self.r)?;
            }
            let drawn = self.frame(at, i + 1 == frames)?;
            if self.bench.overlap {
                flight.submitted(self.r);
            } else {
                self.r.finish()?;
            }
            render.push(started.elapsed().as_secs_f64() * 1e3);
            eval.push(drawn.eval_us);
            if i >= WARMUP {
                let s = self.view.stats();
                (created, steps, compiled) = (created + s.created, steps + s.steps, compiled + s.compiled);
                sim.push(f64::from(s.sim_us) / 1e3);
                gpu.push(f64::from(s.gpu_us) / 1e3);
                sim_gpu.push(f64::from(s.sim_gpu_us) / 1e3);
            }
        }
        self.r.finish()?;
        let (seconds, counted) = (timed.elapsed().as_secs_f64(), frames - WARMUP.min(frames - 1));
        let first_ms = render.first().copied().unwrap_or(0.0);
        let throughput = json!({ "overlap": self.bench.overlap, "frames": counted, "seconds": seconds,
            "fps": counted as f64 / seconds, "ms_per_frame": seconds * 1e3 / counted as f64 });
        Ok(json!({
            "frames": frames, "needed": needed, "eval_us": spread(eval), "render_ms": spread(render),
            "gpu_ms": spread(gpu), "first_ms": first_ms, "created_after_warmup": created,
            "compiled_after_warmup": compiled, "throughput": throughput,
            "particles": { "steps": steps, "sim_ms": spread(sim), "sim_gpu_ms": spread(sim_gpu) },
        }))
    }
}
