use super::super::pipes::STRIDE;
use super::Pool;
use wgpu::BindGroup;

impl Pool {
    pub(crate) fn own(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        views: &[(u64, &wgpu::TextureView)],
    ) -> BindGroup {
        let mut key = [2; 12];
        views.iter().zip(&mut key[1..]).for_each(|((id, _), k)| *k = *id);
        if let Some((group, used)) = self.groups.get_mut(&key) {
            *used = self.frame;
            return group.clone();
        }
        let buffer = self.buffer.as_ref().expect("uploaded before encoding");
        let part = |size| {
            wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: wgpu::BufferSize::new(size),
            })
        };
        let mut entries = vec![wgpu::BindGroupEntry { binding: 0, resource: part(STRIDE) }];
        let textures = views.iter().enumerate().map(|(i, (_, v))| wgpu::BindGroupEntry {
            binding: i as u32 + 1,
            resource: wgpu::BindingResource::TextureView(v),
        });
        entries.extend(textures);
        entries.push(wgpu::BindGroupEntry { binding: 12, resource: part(super::super::stops::MORE) });
        let group =
            device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &entries });
        self.created += 1;
        self.groups.insert(key, (group.clone(), self.frame));
        group
    }

    pub(crate) fn lit(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        views: &[(u64, &wgpu::TextureView); 8],
    ) -> BindGroup {
        let mut key = [3; 12];
        views.iter().zip(&mut key[1..]).for_each(|((id, _), k)| *k = *id);
        if let Some((group, used)) = self.groups.get_mut(&key) {
            *used = self.frame;
            return group.clone();
        }
        let first = self.glow.buffer.as_entire_binding();
        let resources =
            std::iter::once(first).chain(views.iter().map(|(_, v)| wgpu::BindingResource::TextureView(v)));
        let entries: Vec<_> = resources
            .enumerate()
            .map(|(i, resource)| wgpu::BindGroupEntry { binding: i as u32, resource })
            .collect();
        let group =
            device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &entries });
        self.created += 1;
        self.groups.insert(key, (group.clone(), self.frame));
        group
    }

    pub(crate) fn group(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        fx: bool,
        views: &[(u64, &wgpu::TextureView)],
    ) -> BindGroup {
        let mut key = [0; 12];
        key[0] = u64::from(fx);
        views.iter().zip(&mut key[1..]).for_each(|((id, _), k)| *k = *id);
        if let Some((group, used)) = self.groups.get_mut(&key) {
            *used = self.frame;
            return group.clone();
        }
        let buffer = self.buffer.as_ref().expect("uploaded before encoding");
        let uniform = wgpu::BufferBinding { buffer, offset: 0, size: wgpu::BufferSize::new(STRIDE) };
        let first = fx.then_some(wgpu::BindingResource::Buffer(uniform));
        let resources =
            first.into_iter().chain(views.iter().map(|(_, v)| wgpu::BindingResource::TextureView(v)));
        let entries: Vec<_> = resources
            .enumerate()
            .map(|(i, resource)| wgpu::BindGroupEntry { binding: i as u32, resource })
            .collect();
        let group =
            device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &entries });
        self.created += 1;
        self.groups.insert(key, (group.clone(), self.frame));
        group
    }
}
