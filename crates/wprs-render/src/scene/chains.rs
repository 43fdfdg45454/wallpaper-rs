use super::pipes::{Key, Mode, Pipes, Shader, module};
use crate::Renderer;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use wgpu::{ShaderModule, TextureFormat};
use wprs_scene::{CHAIN_ENTRY, Descriptor, Output, Scene, descriptor, effect_source, fusion};

#[derive(Default)]
pub(crate) struct Shelf {
    pub(crate) modules: HashMap<u64, ShaderModule>,
    pub(crate) cache: HashMap<Key, wgpu::RenderPipeline>,
    pub(crate) owned: HashMap<u64, Arc<Descriptor>>,
    pub(crate) held: HashMap<u64, usize>,
}

pub(crate) struct Lease(Vec<u64>, Arc<Mutex<Shelf>>);

pub(crate) fn lock(shelf: &Mutex<Shelf>) -> MutexGuard<'_, Shelf> {
    shelf.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Drop for Lease {
    fn drop(&mut self) {
        let mut shelf = lock(&self.1);
        for key in &self.0 {
            let Some(n) = shelf.held.get_mut(key) else { continue };
            *n -= 1;
            if *n == 0 {
                shelf.held.remove(key);
                shelf.modules.remove(key);
                shelf.owned.remove(key);
                shelf.cache.retain(
                    |(shader, ..), _| !matches!(shader, Shader::Fx(k, _) | Shader::Own(k, ..) if k == key),
                );
            }
        }
    }
}

impl Pipes {
    pub(crate) fn chain<'d>(
        &self,
        device: &wgpu::Device,
        chain: impl Iterator<Item = &'d Descriptor> + Clone,
        lease: Option<&mut Lease>,
    ) -> u64 {
        let key = digest(chain.clone().map(|d| d.id.as_str()));
        let mut shelf = lock(&self.shelf);
        if let Some(lease) = lease.filter(|l| !l.0.contains(&key)) {
            *shelf.held.entry(key).or_default() += 1;
            lease.0.push(key);
        }
        shelf.modules.entry(key).or_insert_with(|| {
            super::pipes::compiled(1);
            module(device, effect_source(&chain.collect::<Vec<_>>()))
        });
        key
    }

    pub(crate) fn hold(&self, key: u64, lease: &mut Lease) {
        let mut shelf = lock(&self.shelf);
        if !lease.0.contains(&key) {
            *shelf.held.entry(key).or_default() += 1;
            lease.0.push(key);
        }
    }

    pub(crate) fn prepare(
        &self,
        device: &wgpu::Device,
        scene: &Scene,
        format: TextureFormat,
        state: TextureFormat,
    ) -> Lease {
        let mut lease = Lease(Vec::new(), self.shelf.clone());
        let base = self.chain(device, std::iter::empty::<&Descriptor>(), Some(&mut lease));
        let _ = self.get(device, Shader::Fx(base, "fs_mask_mix"), Mode::Replace, format);
        let glow = scene.canvas.bloom.as_ref().and_then(|_| descriptor("bloom"));
        let used = scene.drawable().flat_map(|l| l.effects.iter().filter_map(|e| descriptor(&e.effect)));
        for d in used.chain(glow) {
            let key = self.chain(device, std::iter::once(d), Some(&mut lease));
            for pass in &d.passes {
                let target = if pass.output == Output::Next { format } else { state };
                let _ = self.get(device, Shader::Fx(key, &pass.entry), Mode::Replace, target);
            }
        }
        for layer in scene.drawable() {
            for run in fusion(&layer.effects) {
                let chain = layer.effects[run].iter().filter_map(|e| descriptor(&e.effect));
                let key = self.chain(device, chain, Some(&mut lease));
                let _ = self.get(device, Shader::Fx(key, CHAIN_ENTRY), Mode::Replace, format);
            }
        }
        lease
    }
}

fn digest<'a>(ids: impl Iterator<Item = &'a str>) -> u64 {
    let mut h = DefaultHasher::new();
    ids.for_each(|id| id.hash(&mut h));
    h.finish()
}

impl Renderer {
    pub fn effect_shader(&self, chain: &[&str]) -> bool {
        lock(&self.pipes().shelf).modules.contains_key(&digest(chain.iter().copied()))
    }
}
