use super::chains::{Shelf, lock};
use super::flight::building;
use super::make::Recipe;
use super::pipes::{Key, Shader};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use wgpu::BindGroupLayout;
use wprs_scene::Descriptor;

const ROOM: usize = 64 << 20;
const WORKERS: usize = 4;

pub(crate) struct Job {
    pub(crate) d: Arc<Descriptor>,
    pub(crate) wanted: Vec<Key>,
    pub(crate) variants: Vec<u32>,
}

pub(crate) struct Kit {
    pub(crate) device: wgpu::Device,
    pub(crate) groups: [BindGroupLayout; 2],
    pub(crate) shelf: Arc<Mutex<Shelf>>,
}

pub(crate) type Built = (u64, Result<(), String>);

impl Kit {
    pub(crate) fn start(
        self,
        jobs: Vec<Job>,
        stop: Arc<AtomicBool>,
    ) -> Result<mpsc::Receiver<Built>, String> {
        let (tx, rx) = mpsc::channel();
        let workers = jobs.len().min(WORKERS);
        let (kit, jobs) = (Arc::new(self), Arc::new(Mutex::new(VecDeque::from(jobs))));
        for _ in 0..workers {
            let (kit, jobs, halt, tx) = (kit.clone(), jobs.clone(), stop.clone(), tx.clone());
            let work = move || {
                loop {
                    let next = jobs.lock().unwrap_or_else(PoisonError::into_inner).pop_front();
                    let Some(job) = next.filter(|_| !halt.load(Relaxed)) else { break };
                    let key = job.d.key;
                    building(key, true);
                    drop(wprs_scene::trying(key));
                    let built = kit.build(&job);
                    building(key, false);
                    if tx.send((key, built)).is_err() {
                        break;
                    }
                }
            };
            let thread = std::thread::Builder::new().name("wprs-build".into()).stack_size(ROOM);
            let thread = thread.spawn(work).map_err(|e| format!("cannot start building it: {e}"))?;
            super::builders::keep(thread, stop.clone());
        }
        Ok(rx)
    }

    fn build(&self, job: &Job) -> Result<(), String> {
        let key = job.d.key;
        let have = lock(&self.shelf).modules.get(&key).cloned();
        let parsed = match have {
            Some(_) => None,
            None => Some(wprs_scene::guarded(&job.d)?),
        };
        let ((module, made), failure) = crate::lost::caught(&self.device, || {
            let module = have.unwrap_or_else(|| {
                let source = wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(parsed.expect("parsed above")));
                let label = Some("wprs-scene package shader");
                self.device.create_shader_module(wgpu::ShaderModuleDescriptor { label, source })
            });
            let made: Vec<(Key, wgpu::RenderPipeline)> = (job.wanted.iter())
                .map(|&(shader, mode, format)| {
                    let recipe = Recipe::own(&job.d, module.clone(), self.groups.clone(), shader);
                    ((shader, mode, format), recipe.make(&self.device, mode, format))
                })
                .collect();
            (module, made)
        });
        if let Some(e) = failure {
            return Err(e.to_string());
        }
        let mut shelf = lock(&self.shelf);
        if shelf.held.contains_key(&key) {
            shelf.modules.entry(key).or_insert(module);
            shelf.owned.entry(key).or_insert_with(|| job.d.clone());
            for (k, p) in made {
                shelf.cache.entry(k).or_insert(p);
            }
        }
        Ok(())
    }
}

pub(crate) fn own(key: u64, pass: usize, variant: u32) -> Shader {
    Shader::Own(key, u8::try_from(pass).unwrap_or(u8::MAX), variant)
}
