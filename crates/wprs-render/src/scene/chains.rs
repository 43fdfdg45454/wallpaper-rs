use super::pipes::{Key, Mode, Pipes, Shader, module};
use crate::Renderer;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use wgpu::{ShaderModule, TextureFormat};
use wprs_scene::{CHAIN_ENTRY, Descriptor, Output, Scene, descriptor, effect_source, fusion};

#[derive(Default)]
pub(crate) struct Shelf {
    pub(crate) modules: HashMap<u64, ShaderModule>,
    pub(crate) cache: HashMap<Key, wgpu::RenderPipeline>,
    held: HashMap<u64, usize>,
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
                shelf.cache.retain(|(shader, ..), _| !matches!(shader, Shader::Fx(k, _) if k == key));
            }
        }
    }
}

impl Pipes {
    pub(crate) fn chain<'d>(
        &self,
        device: &wgpu::Device,
        chain: impl Iterator<Item = &'d Descriptor> + Clone,
        hold: bool,
    ) -> u64 {
        let mut h = DefaultHasher::new();
        chain.clone().for_each(|d| d.id.hash(&mut h));
        let key = h.finish();
        let mut shelf = lock(&self.shelf);
        if hold {
            *shelf.held.entry(key).or_default() += 1;
        }
        shelf.modules.entry(key).or_insert_with(|| {
            self.created.fetch_add(1, Ordering::Relaxed);
            module(device, effect_source(&chain.collect::<Vec<_>>()))
        });
        key
    }

    pub(crate) fn prepare(
        &self,
        device: &wgpu::Device,
        scene: &Scene,
        format: TextureFormat,
        state: TextureFormat,
    ) -> Lease {
        let base = self.chain(device, std::iter::empty::<&Descriptor>(), false);
        let _ = self.get(device, Shader::Fx(base, "fs_mask_mix"), Mode::Replace, format);
        let mut held = Vec::new();
        for layer in scene.layers() {
            for d in layer.effects.iter().filter_map(|e| descriptor(&e.effect)) {
                let key = self.chain(device, std::iter::once(d), false);
                for pass in &d.passes {
                    let target = if pass.output == Output::State { state } else { format };
                    let _ = self.get(device, Shader::Fx(key, &pass.entry), Mode::Replace, target);
                }
            }
            for run in fusion(&layer.effects) {
                let key =
                    self.chain(device, layer.effects[run].iter().filter_map(|e| descriptor(&e.effect)), true);
                let _ = self.get(device, Shader::Fx(key, CHAIN_ENTRY), Mode::Replace, format);
                held.push(key);
            }
        }
        Lease(held, self.shelf.clone())
    }
}

impl Renderer {
    pub fn effect_shaders(&self) -> usize {
        lock(&self.pipes().shelf).modules.len()
    }
}
