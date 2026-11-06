use super::dispatch::Job;
use super::{SIM, Swarms};
use crate::Renderer;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Phase {
    Simulate(u32),
    Emit(u32),
    Settle,
    Twine,
}

pub(super) struct Task {
    pub(super) herd: usize,
    pub(super) p: usize,
    pub(super) k: usize,
    pub(super) depth: u32,
    pub(super) cells: u32,
    pub(super) threads: u32,
    pub(super) settle: bool,
    pub(super) twine: bool,
}

type Pass = (Option<wgpu::ComputePass<'static>>, u32);

fn open<'a>(
    pass: &'a mut Pass,
    stamp: &mut super::Stamp,
    r: &Renderer,
) -> &'a mut wgpu::ComputePass<'static> {
    pass.1 += 1;
    pass.0.get_or_insert_with(|| stamp.encoder(r).begin_compute_pass(&Default::default()).forget_lifetime())
}

fn close((pass, stores): Pass, stamp: &mut super::Stamp) {
    drop(pass);
    if stores > 1 {
        stamp.cut();
    }
}

impl Swarms {
    pub(super) fn record(&mut self, r: &Renderer, tasks: &[Task], k: usize, phase: Phase) {
        let Swarms { herds, flights, compute, flocking, twining, stamp, .. } = self;
        let Some([simulate, emit, settle]) = compute.as_ref() else { return };
        let mut pass: Pass = (None, 0);
        for t in tasks.iter().filter(|t| t.k == k || phase == Phase::Twine) {
            let Some(store) = flights[t.herd].as_ref().and_then(|f| f.stores.get(t.p)) else { continue };
            let Some((_, _, group)) = &store.gpu else { continue };
            let (offset, alloc) = ([t.k as u32 * SIM as u32], herds[t.herd].bake.allocs[t.p]);
            match phase {
                Phase::Simulate(d) if t.depth == d => {
                    let pass = open(&mut pass, stamp, r);
                    if let (Some(flock), Some([clear, tally, scan, scatter, steer])) =
                        (&store.flock, flocking.as_ref())
                    {
                        pass.set_bind_group(0, flock, &offset);
                        let each = alloc.div_ceil(64);
                        let steps = [(clear, t.cells.div_ceil(64)), (tally, each), (scan, 1)];
                        for (pipe, n) in steps.into_iter().chain([(scatter, each), (steer, each)]) {
                            pass.set_pipeline(pipe);
                            pass.dispatch_workgroups(n, 1, 1);
                        }
                    }
                    pass.set_bind_group(0, group, &offset);
                    pass.set_pipeline(simulate);
                    pass.dispatch_workgroups(alloc.div_ceil(64), 1, 1);
                }
                Phase::Emit(d) if t.depth == d && t.threads > 0 => {
                    let pass = open(&mut pass, stamp, r);
                    pass.set_bind_group(0, group, &offset);
                    pass.set_pipeline(emit);
                    pass.dispatch_workgroups(t.threads.div_ceil(64), 1, 1);
                }
                Phase::Settle if t.settle => {
                    let pass = open(&mut pass, stamp, r);
                    pass.set_bind_group(0, group, &offset);
                    pass.set_pipeline(settle);
                    pass.dispatch_workgroups(1, 1, 1);
                }
                Phase::Twine if t.twine && store.rope.is_some() => {
                    let (Some(twine), Some(group)) = (twining.as_ref(), store.twine.as_ref()) else {
                        continue;
                    };
                    let pass = open(&mut pass, stamp, r);
                    pass.set_bind_group(0, group, &offset);
                    pass.set_pipeline(twine);
                    pass.dispatch_workgroups(1, 1, 1);
                }
                _ => {}
            }
        }
        close(pass, stamp);
    }

    pub(super) fn sort_all(&mut self, r: &Renderer, jobs: &[Job]) {
        let Swarms { herds, flights, sorts, stamp, .. } = self;
        let Some(pipes) = sorts.as_ref() else { return };
        for (n, pipe) in pipes.iter().enumerate() {
            let mut pass: Pass = (None, 0);
            for job in jobs {
                let Some(flight) = &flights[job.herd] else { continue };
                for (p, store) in flight.stores.iter().enumerate() {
                    for (_, group) in store.stacks.iter().filter_map(|s| s.gpu.as_ref()) {
                        let pass = open(&mut pass, stamp, r);
                        pass.set_bind_group(0, group, &[]);
                        pass.set_pipeline(pipe);
                        let groups = if n == 1 { 1 } else { herds[job.herd].bake.allocs[p].div_ceil(64) };
                        pass.dispatch_workgroups(groups, 1, 1);
                    }
                }
            }
            close(pass, stamp);
        }
    }
}
