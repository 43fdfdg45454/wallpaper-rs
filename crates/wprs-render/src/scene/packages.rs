use super::build::{Job, Kit, own};
use super::chains::{Lease, lock};
use super::pipes::{Key, Shader};
use super::stops::{UNBUILT, stopped};
use crate::Renderer;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, mpsc::RecvTimeoutError};
use std::time::{Duration, Instant};
use wgpu::TextureFormat;
use wprs_scene::{Descriptor, Scene};

pub const MAX_SCENE_BUILD: u64 = 1 << 18;
static TIMEOUT_MS: AtomicU64 = AtomicU64::new(10_000);

pub fn set_build_timeout(limit: Duration) {
    TIMEOUT_MS.store(u64::try_from(limit.as_millis()).unwrap_or(u64::MAX), Relaxed);
}

pub(crate) fn build_timeout() -> Duration {
    Duration::from_millis(TIMEOUT_MS.load(Relaxed))
}

type Refused = BTreeMap<String, String>;

impl Renderer {
    pub(crate) fn own_pipes(
        &self,
        scene: &Scene,
        formats: [TextureFormat; 2],
        lease: &mut Lease,
        view: usize,
    ) -> Refused {
        let mut refused = Refused::new();
        let mut jobs: Vec<Job> = Vec::new();
        let uses = scene.drawable().flat_map(|l| &l.effects);
        for e in uses {
            let Some(d) = scene.loaded.get(&e.effect).filter(|_| !refused.contains_key(&e.effect)) else {
                continue;
            };
            if stopped(d.key) {
                refused.insert(d.id.clone(), UNBUILT.into());
                continue;
            }
            if let Some(why) = super::clock::slow(d.key) {
                refused.insert(d.id.clone(), why);
                continue;
            }
            match d.variants(e) {
                Ok(variants) => self.want(d, &variants, formats, lease, &mut jobs),
                Err(problem) => drop(refused.insert(d.id.clone(), problem)),
            }
        }
        let mut left = MAX_SCENE_BUILD;
        jobs.retain(|job| {
            let cost = job.d.size.saturating_mul(job.variants.len() as u64);
            let fits = cost <= left;
            left = left.saturating_sub(cost);
            if !fits {
                let why = format!(
                    "the package shaders of the scene are more than {MAX_SCENE_BUILD} operations long"
                );
                refused.insert(job.d.id.clone(), why);
            }
            fits && !job.wanted.is_empty()
        });
        self.run(jobs, &mut refused, view);
        refused
    }

    fn want(
        &self,
        d: &Arc<Descriptor>,
        variants: &[u32],
        formats: [TextureFormat; 2],
        lease: &mut Lease,
        jobs: &mut Vec<Job>,
    ) {
        let pipes = self.pipes();
        pipes.hold(d.key, lease);
        let plain = pipes.chain(&self.device, std::iter::empty::<&Descriptor>(), Some(&mut *lease));
        let mut wanted: Vec<Key> = Vec::new();
        for (n, pass) in d.passes.iter().enumerate() {
            let format = super::ready::format(d, pass, formats);
            let mode = super::own::mode(pass.blend);
            match (&pass.copy, &pass.swap) {
                (Some(_), _) => drop(pipes.get(&self.device, Shader::Fx(plain, "fs_copy"), mode, format)),
                (_, Some(_)) => {}
                _ => wanted.extend(variants.iter().map(|v| (own(d.key, n, *v), mode, format))),
            }
        }
        let shelf = lock(&pipes.shelf);
        wanted.retain(|k| !shelf.cache.contains_key(k));
        drop(shelf);
        let job = match jobs.iter_mut().position(|j| j.d.key == d.key) {
            Some(at) => &mut jobs[at],
            None => {
                jobs.push(Job { d: d.clone(), wanted: Vec::new(), variants: Vec::new() });
                jobs.last_mut().expect("pushed above")
            }
        };
        for v in variants {
            if !job.variants.contains(v) {
                job.variants.push(*v);
            }
        }
        for k in wanted {
            if !job.wanted.contains(&k) {
                job.wanted.push(k);
            }
        }
    }

    fn run(&self, jobs: Vec<Job>, refused: &mut Refused, view: usize) {
        if jobs.is_empty() {
            return;
        }
        let pipes = self.pipes();
        let groups = [pipes.fx_frame.clone(), pipes.own.clone()];
        let kit = Kit { device: self.device.clone(), groups, shelf: pipes.shelf.clone() };
        let mut waiting: Vec<(u64, String)> = jobs.iter().map(|j| (j.d.key, j.d.id.clone())).collect();
        let stop = Arc::new(AtomicBool::new(false));
        let found = match kit.start(jobs, stop.clone()) {
            Ok(found) => found,
            Err(e) => return waiting.into_iter().for_each(|(_, id)| drop(refused.insert(id, e.clone()))),
        };
        let until = Instant::now() + Duration::from_millis(TIMEOUT_MS.load(Relaxed));
        while !waiting.is_empty() {
            let (key, built) = match found.recv_timeout(until.saturating_duration_since(Instant::now())) {
                Ok(message) => message,
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            };
            let Some(at) = waiting.iter().position(|(k, _)| *k == key) else { continue };
            let (_, id) = waiting.swap_remove(at);
            match built {
                Ok(()) => super::flight::pending(view, key),
                Err(e) => {
                    wprs_scene::tried(key);
                    tracing::warn!("the effect {id} was left out: {e}");
                    refused.insert(id, e);
                }
            }
        }
        stop.store(true, Relaxed);
        for (key, id) in waiting {
            let seconds = TIMEOUT_MS.load(Relaxed) as f32 / 1000.0;
            let why = format!("its shader took more than {seconds} s to build");
            if !super::flight::built(key) && wprs_scene::strike(&[key]).is_err() {
                tracing::warn!("cannot remember that the effect {id} took too long to build");
            }
            tracing::warn!("the effect {id} was left out: {why}");
            refused.insert(id, why);
        }
    }
}
