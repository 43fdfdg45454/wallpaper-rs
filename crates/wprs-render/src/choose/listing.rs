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
        self.list(&self.instance)
    }

    pub fn plugged(&self) -> Vec<GpuAdapter> {
        let own = match self.gpu.get_info().backend {
            Backend::Gl => Backends::GL,
            _ => Backends::VULKAN,
        };
        self.list(&instance(Backends::from_env().unwrap_or(own), None))
    }

    fn list(&self, base: &wgpu::Instance) -> Vec<GpuAdapter> {
        let own = self.gpu.get_info();
        let mut found: Vec<wgpu::AdapterInfo> =
            listed(base, Backends::all()).iter().map(wgpu::Adapter::get_info).collect();
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
            kind: kind(super::sysfs::kind_of(i)).into(),
            backend: backend(i.backend),
            used: same(i),
            bus: i.device_pci_bus_id.to_lowercase(),
            compositor: false,
        };
        found.iter().map(made).collect()
    }
}
