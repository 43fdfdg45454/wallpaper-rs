use super::damage::Damage;
use super::item::{Item, Rect, frame_data, linear};
use super::paint::Painter;
use super::pool::{Pool, Tex};
use super::{SceneStats, SceneView, Src, TEXT_FRAMES, encode, pipes, timed};
use crate::texture::GpuImage;
use crate::{RenderError, Renderer};
use std::hash::{Hash, Hasher};
use wgpu::TextureFormat as Format;
use wprs_scene::{Frame, Quality, Runtime};

fn retarget(r: &Renderer, view: &mut SceneView, size: (u32, u32)) {
    let pool = &mut view.pool;
    view.caches.drain().for_each(|(_, c)| pool.give(c.tex));
    if let Some((canvas, output)) = view.target.take() {
        let GpuImage { texture, view, id } = output;
        if id != canvas.id {
            pool.give(Tex { texture, view, id });
        }
        pool.give(canvas);
    }
    let canvas = pool.take(&r.device, size, view.format);
    let output = match view.srgb {
        true => {
            let Tex { texture, view, id } = pool.take(&r.device, size, Format::Rgba8UnormSrgb);
            GpuImage { texture, view, id }
        }
        false => GpuImage { texture: canvas.texture.clone(), view: canvas.view.clone(), id: canvas.id },
    };
    view.target = Some((canvas, output));
    view.marks.clear();
}

fn resolve(pool: &mut Pool) -> u32 {
    let (m0, m1, full) = ([2.0, 0.0, -1.0, 0.0], [0.0, -2.0, 1.0, 0.0], [0.0, 0.0, 1.0, 1.0]);
    let mode = [0.0, 1.0, 0.0, 0.0];
    pool.push(&Item { m0, m1, quad: full, uv: full, color: [1.0; 4], mode, ..Default::default() })
}

impl Renderer {
    pub fn render_scene<'v>(
        &self,
        view: &'v mut SceneView,
        runtime: &Runtime,
        frame: &Frame<'_>,
        size: (u32, u32),
        quality: Quality,
        videos: &[(&str, &GpuImage)],
    ) -> Result<&'v GpuImage, RenderError> {
        self.check_target(size.0, size.1)?;
        self.judge(view, frame);
        let created = (view.pool.created, pipes::compiled(0));
        if view.target.as_ref().is_none_or(|(t, _)| t.size() != size) {
            retarget(self, view, size);
        }
        let screen = view.screen(size);
        let data = frame_data(runtime, frame, size, view.srgb, quality);
        self.queue.write_buffer(&view.uniform, 0, bytemuck::bytes_of(&data));
        self.shrunk(view, frame, screen);
        let mut scratch = std::mem::take(&mut view.scratch);
        let s = &mut scratch;
        s.ops.clear();
        s.exts.clear();
        s.kept.clear();
        s.outputs.clear();
        s.texts.clear();
        s.drawn.clear();
        s.sets.clear();
        s.made.clear();
        s.isolated.clear();
        s.chains.clear();
        s.chains.resize(frame.draws.len(), None);
        self.rasters(view, frame, screen, &mut scratch.texts);
        if let Some(swarms) = view.swarms.as_mut() {
            self.scoped(|| swarms.run(self, runtime, frame, quality))??;
        }
        let v = &mut *view;
        let (srgb, format, state_format) = (v.srgb, v.format, v.state_format);
        let full = Rect { x: 0.0, y: 0.0, w: size.0 as f32, h: size.1 as f32 };
        let mut painter = Painter {
            r: self,
            frame,
            pool: &mut v.pool,
            caches: &mut v.caches,
            states: &mut v.states,
            images: &v.images,
            samplings: &v.samplings,
            luts: &v.luts,
            texts: &scratch.texts,
            rasters: &v.texts,
            videos,
            exts: &mut scratch.exts,
            screen,
            stage: screen.then(wprs_scene::Affine::scale(v.canvas.map(|v| v as f32))),
            full,
            clip: None,
            moment: {
                let mut h = std::hash::DefaultHasher::new();
                bytemuck::bytes_of(&super::item::FrameData { dt: 0.0, ..data }).hash(&mut h);
                h.finish()
            },
            srgb,
            format,
            state_format,
            ops: &mut scratch.ops,
            reused: 0,
            kept: &mut scratch.kept,
            outputs: &mut scratch.outputs,
            lease: &mut v._lease,
            swarms: v.swarms.as_mut(),
            refused: &mut v.refused,
            drawn: &mut scratch.drawn,
            sets: &mut scratch.sets,
            made: &mut scratch.made,
            isolated: &mut scratch.isolated,
            chains: &mut scratch.chains,
            paints: &v.paints,
            more: super::stops::frame(runtime),
            backdrop: None,
        };
        let c = frame.background.0;
        let background = if srgb { c } else { [linear(c[0]), linear(c[1]), linear(c[2]), c[3]] };
        let stamp = {
            let mut h = std::hash::DefaultHasher::new();
            (size, background.map(f32::to_bits), quality as u8, srgb).hash(&mut h);
            h.finish()
        };
        let damage =
            painter.damage(&v.marks, &mut scratch.marks, std::mem::replace(&mut v.stamp, stamp) != stamp);
        std::mem::swap(&mut v.marks, &mut scratch.marks);
        if damage == Damage::Clean {
            let (steps, particles, sim_us) =
                v.swarms.as_ref().map_or((0, 0, 0), |s| (s.steps, s.slots(), s.micros));
            let in_flight = super::flight::flying(v.id);
            v.stats = SceneStats { steps, particles, sim_us, in_flight, ..SceneStats::default() };
            v.pool.end_frame();
            super::flight::idle(v.id);
            v.scratch = scratch;
            return Ok(&view.target.as_ref().expect("created above").1);
        }
        painter.clip = match damage {
            Damage::Part(rect) => Some(rect),
            _ => None,
        };
        painter.run(Src::Canvas, background.map(f64::from));
        let clip = painter.clip;
        let reused = painter.reused;
        let (trial, drawn) = (super::flight::trial(v.id), !scratch.drawn.is_empty());
        let timing = v.clock.start(self, &scratch.ops, trial, drawn);
        let serial = super::flight::sending(self, v.id, &scratch.drawn)
            .map_err(|keys| timed::stuck(&mut v.refused, frame, keys))?;
        let quiet = self.quiet(timing);
        let (pool, kept) = (&mut v.pool, &scratch.kept);
        v.caches.extract_if(|id, _| !kept.contains(id)).for_each(|(_, c)| pool.give(c.tex));
        let resolve = srgb.then(|| resolve(pool));
        pool.upload(&self.device, &self.queue, self.pipes(), &v.uniform);
        v.swarms.iter_mut().for_each(|s| s.upload(self));
        let (canvas, output) = v.target.as_ref().expect("created above");
        let (caches, states, luts, swarms) = (&v.caches, &v.states, &v.luts, v.swarms.as_ref());
        let exts = &scratch.exts;
        let scissor = clip.map(|c| [c.x as u32, c.y as u32, c.w as u32, c.h as u32]);
        let sets = &scratch.sets;
        let sources = encode::Sources { caches, states, luts, exts, canvas, output, swarms, scissor, sets };
        let clock = (&v.clock, timing);
        let encoded =
            self.scoped(|| self.encode_scene(pool, &sources, &scratch.ops, resolve, clock, (v.id, serial)));
        let index = encoded.as_ref().ok().map(|(_, index, _)| index.clone());
        super::flight::sent(self, v.id, serial, index)
            .map_err(|keys| timed::stuck(&mut v.refused, frame, keys))?;
        if let Err(e) = &encoded {
            let ids = frame.effects.iter().filter(|e| scratch.drawn.contains(&e.descriptor.key));
            ids.for_each(|d| drop(v.refused.insert(d.descriptor.id.clone(), e.to_string())));
        }
        let (mut counted, index, at) = encoded?;
        if quiet {
            self.watch((&mut v.clock, &mut v.refused), frame, &scratch.drawn, (index, at));
        }
        let painted = pool.frame;
        let old = |_: &usize, sim: &mut super::state::Sim| painted - sim.used >= TEXT_FRAMES;
        v.states.extract_if(old).for_each(|(_, sim)| sim.tex.into_iter().for_each(|t| pool.give(t)));
        pool.end_frame();
        let now = pool.frame;
        v.texts.retain(|_, (.., used)| now - *used < TEXT_FRAMES);
        counted.compiled = pipes::compiled(0).wrapping_sub(created.1);
        (counted.created, counted.reused, counted.textures) =
            (pool.created - created.0 + counted.compiled, reused, pool.textures() as u32);
        counted.damage = clip.map_or(100, |c| ((c.w * c.h * 100.0) / (full.w * full.h)).ceil() as u32);
        if let Some(s) = swarms {
            (counted.created, counted.steps) = (counted.created + s.created, s.steps);
            (counted.particles, counted.sim_us) = (s.slots(), s.micros);
        }
        counted.in_flight = super::flight::flying(v.id);
        v.stats = counted;
        v.scratch = scratch;
        Ok(&view.target.as_ref().expect("created above").1)
    }
}
