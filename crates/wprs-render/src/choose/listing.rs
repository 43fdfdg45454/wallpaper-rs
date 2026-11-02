use super::{backend, kind, listed};
use crate::Renderer;
use crate::adapter::instance;
use wgpu::{Backend, Backends};
use wprs_core::model::GpuAdapter;

pub(crate) fn extra(own: Backend, limit: Option<Backends>) -> Option<Backends> {
    (limit.is_none() && own != Backend::Vulkan).then_some(Backends::VULKAN)
}

impl Renderer {
    pub fn adapters(&self) -> Vec<GpuAdapter> {
        let own = self.gpu.get_info();
        let mut found: Vec<wgpu::AdapterInfo> =
            listed(&self.instance, Backends::all()).iter().map(wgpu::Adapter::get_info).collect();
        if let Some(more) = extra(own.backend, Backends::from_env()) {
            found.extend(listed(&instance(more, None), more).iter().map(wgpu::Adapter::get_info));
        }
        let same = |i: &wgpu::AdapterInfo| {
            (&i.name, i.vendor, i.device, &i.device_pci_bus_id, i.backend)
                == (&own.name, own.vendor, own.device, &own.device_pci_bus_id, own.backend)
        };
        if !found.iter().any(same) {
            found.push(own.clone());
        }
        let made = |i: &wgpu::AdapterInfo| GpuAdapter {
            name: i.name.clone(),
            kind: kind(i.device_type).into(),
            backend: backend(i.backend),
            used: same(i),
        };
        found.iter().map(made).collect()
    }
}
