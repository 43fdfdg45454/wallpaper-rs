use crate::adapter::{instance, open_auto};
use crate::device::Display;
use crate::{RenderError, Renderer};
use std::sync::Arc;
use wgpu::wgt::WgpuHasDisplayHandle;
use wgpu::{Backend, Backends, DeviceType};
use wprs_core::GpuChoice;
use wprs_core::model::GpuAdapter;

mod listing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fallback {
    Missing,
    Unusable,
}

pub struct Chosen {
    pub renderer: Renderer,
    pub adapters: Vec<GpuAdapter>,
    pub fallback: Option<Fallback>,
}

pub(crate) fn kind(t: DeviceType) -> &'static str {
    match t {
        DeviceType::IntegratedGpu => "integrated",
        DeviceType::DiscreteGpu => "dedicated",
        DeviceType::VirtualGpu => "virtual",
        DeviceType::Cpu => "software",
        DeviceType::Other => "other",
    }
}

pub(crate) fn backend(b: Backend) -> String {
    match b {
        Backend::Gl => "gl".into(),
        other => format!("{other:?}").to_lowercase(),
    }
}

pub(crate) fn pick(choice: &GpuChoice, found: &[(String, DeviceType, Backend)]) -> Option<usize> {
    let fits = |(name, t, _): &(String, DeviceType, Backend)| match choice {
        GpuChoice::Auto => false,
        GpuChoice::Integrated => *t == DeviceType::IntegratedGpu,
        GpuChoice::Dedicated => *t == DeviceType::DiscreteGpu,
        GpuChoice::Software => *t == DeviceType::Cpu,
        GpuChoice::Named(part) => name.to_lowercase().contains(&part.to_lowercase()),
    };
    let found = found.iter().enumerate().filter(|(_, a)| fits(a));
    found.min_by_key(|(i, (.., b))| (*b != Backend::Vulkan, *i)).map(|(i, _)| i)
}

pub(crate) fn listed(instance: &wgpu::Instance, backends: Backends) -> Vec<wgpu::Adapter> {
    pollster::block_on(instance.enumerate_adapters(backends))
}

fn described(adapters: &[wgpu::Adapter]) -> Vec<(String, DeviceType, Backend)> {
    adapters.iter().map(|a| a.get_info()).map(|i| (i.name, i.device_type, i.backend)).collect()
}

fn hardware(adapters: &[wgpu::Adapter]) -> bool {
    adapters.iter().any(|a| a.get_info().device_type != DeviceType::Cpu)
}

pub(crate) fn settle<T>(opened: Option<Result<T, RenderError>>, choice: &str) -> Result<T, Fallback> {
    match opened {
        Some(Ok(done)) => Ok(done),
        Some(Err(e)) => {
            tracing::warn!(
                "the graphics card chosen by render.gpu = {choice} cannot be opened; using the automatic choice"
            );
            tracing::debug!("{e}");
            Err(Fallback::Unusable)
        }
        None => {
            tracing::warn!("no graphics card matches render.gpu = {choice}; using the automatic choice");
            Err(Fallback::Missing)
        }
    }
}

type Found = (wgpu::Instance, wgpu::Adapter);

fn find(choice: &GpuChoice, display: Option<&Arc<dyn WgpuHasDisplayHandle>>) -> Option<Found> {
    let first = Backends::from_env().unwrap_or(Backends::VULKAN);
    let vulkan = instance(first, display);
    let adapters = listed(&vulkan, first);
    let found = pick(choice, &described(&adapters)).map(|i| (vulkan, adapters[i].clone()));
    let soft = first == Backends::VULKAN && (*choice == GpuChoice::Software || !hardware(&adapters));
    found.or_else(|| {
        let gl = soft.then(|| instance(Backends::GL, display))?;
        let adapters = listed(&gl, Backends::GL);
        pick(choice, &described(&adapters)).map(|i| (gl, adapters[i].clone()))
    })
}

impl Renderer {
    pub fn with_choice(display: Option<Display>, choice: &GpuChoice) -> Result<Chosen, RenderError> {
        let display: Option<Arc<dyn WgpuHasDisplayHandle>> = display.map(Arc::from);
        let mut fallback = None;
        if !choice.is_auto() {
            let opened = find(choice, display.as_ref()).map(|(i, a)| Self::with_adapter(i, a));
            match settle(opened, choice.as_str()) {
                Ok(renderer) => return Ok(renderer.chosen(None)),
                Err(why) => fallback = Some(why),
            }
        }
        let (instance, adapter) = open_auto(display.as_ref())?;
        Self::with_adapter(instance, adapter).map(|r| r.chosen(fallback))
    }

    fn chosen(self, fallback: Option<Fallback>) -> Chosen {
        let adapters = self.adapters();
        Chosen { renderer: self, adapters, fallback }
    }
}

#[cfg(test)]
mod tests;
