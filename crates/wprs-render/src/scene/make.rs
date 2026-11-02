use super::chains::lock;
use super::pipes::{Mode, Pipes, Shader, blend, compiled};
use wgpu::{BindGroupLayout, ShaderModule, TextureFormat};
use wprs_scene::Descriptor;

pub(crate) struct Recipe {
    module: ShaderModule,
    entry: String,
    vertex: String,
    groups: Vec<BindGroupLayout>,
    constants: Vec<(String, f64)>,
    strip: bool,
}

impl Recipe {
    pub(crate) fn own(
        d: &Descriptor,
        module: ShaderModule,
        groups: Vec<BindGroupLayout>,
        shader: Shader,
    ) -> Recipe {
        let Shader::Own(_, pass, variant) = shader else { unreachable!("a package shader") };
        let p = &d.passes[usize::from(pass)];
        let (entry, vertex) = (p.entry.clone(), p.vertex.clone().unwrap_or_else(|| "vs_main".into()));
        Recipe { module, entry, vertex, groups, constants: d.constants(variant), strip: false }
    }

    pub(crate) fn make(
        &self,
        device: &wgpu::Device,
        mode: Mode,
        format: TextureFormat,
    ) -> wgpu::RenderPipeline {
        let constants: Vec<(&str, f64)> = self.constants.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        let options = wgpu::PipelineCompilationOptions { constants: &constants, ..Default::default() };
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("wprs-scene pipeline layout"),
            bind_group_layouts: &self.groups.iter().map(Some).collect::<Vec<_>>(),
            immediate_size: 0,
        });
        let topology = match self.strip {
            true => wgpu::PrimitiveTopology::TriangleStrip,
            false => wgpu::PrimitiveTopology::TriangleList,
        };
        let target = wgpu::ColorTargetState { blend: blend(mode), ..format.into() };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&self.entry),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &self.module,
                entry_point: Some(&self.vertex),
                compilation_options: options.clone(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.module,
                entry_point: Some(&self.entry),
                compilation_options: options,
                targets: &[Some(target)],
            }),
            primitive: wgpu::PrimitiveState { topology, ..Default::default() },
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        compiled(1);
        pipeline
    }
}

impl Pipes {
    pub(crate) fn get(
        &self,
        device: &wgpu::Device,
        shader: Shader,
        mode: Mode,
        format: TextureFormat,
    ) -> Option<wgpu::RenderPipeline> {
        let key = (shader, mode, format);
        let recipe = {
            let shelf = lock(&self.shelf);
            if let Some(found) = shelf.cache.get(&key) {
                return Some(found.clone());
            }
            let module = |id| shelf.modules.get(&id).expect("created first").clone();
            let plain = |module, entry: &str, vertex: &str, groups, strip| Recipe {
                module,
                entry: entry.into(),
                vertex: vertex.into(),
                groups,
                constants: Vec::new(),
                strip,
            };
            match shader {
                Shader::Scene(entry) => {
                    let groups = vec![self.frame.clone(), self.textures.clone()];
                    plain(self.composite.clone(), entry, "vs_item", groups, true)
                }
                Shader::Lit(entry) => {
                    let groups = vec![self.frame.clone(), self.textures.clone(), self.lit.clone()];
                    plain(self.composite.clone(), entry, "vs_item", groups, true)
                }
                Shader::Fx(id, entry) => {
                    plain(module(id), entry, "vs_main", vec![self.fx_frame.clone(), self.fx.clone()], false)
                }
                Shader::Own(..) => return None,
            }
        };
        let pipeline = recipe.make(device, mode, format);
        Some(lock(&self.shelf).cache.entry(key).or_insert(pipeline).clone())
    }
}
