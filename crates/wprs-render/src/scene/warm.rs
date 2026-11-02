use super::pipes::{Mode, Pipes, Shader};
use std::collections::BTreeSet;
use wgpu::TextureFormat as Format;
use wprs_scene::{Blend, Kind, Layer, Scene};

fn mode(blend: Blend, atop: bool) -> Mode {
    match blend {
        Blend::Normal if atop => Mode::Atop,
        Blend::Normal => Mode::Normal,
        Blend::Add => Mode::Add,
        Blend::Multiply => Mode::Multiply,
        Blend::Screen => Mode::Screen,
        _ => Mode::Replace,
    }
}

fn entry(layer: &Layer) -> Option<&'static str> {
    match &layer.kind {
        Kind::Image { .. } | Kind::Video { .. } => Some("fs_picture"),
        Kind::Text(_) | Kind::Group { .. } | Kind::Adjustment { .. } => Some("fs_texture"),
        Kind::Shape { .. } => Some("fs_shape"),
        Kind::Solid { .. } => Some("fs_solid"),
        Kind::Spectrum(_) => Some("fs_spectrum"),
        _ => None,
    }
}

fn walk(layers: &[Layer], atop: bool, pairs: &mut BTreeSet<(&'static str, Mode)>) {
    for (n, layer) in layers.iter().enumerate() {
        let m = mode(layer.blend, atop && n > 0);
        let styled = matches!(&layer.kind, Kind::Text(t) if t.styled());
        let text = [matches!(layer.kind, Kind::Text(_)).then_some("fs_shape"), styled.then_some("fs_text")];
        for e in entry(layer).into_iter().chain(text.into_iter().flatten()) {
            pairs.extend([(e, m), (e, Mode::Normal), ("fs_texture", m)]);
        }
        if layer.lighting.is_some() {
            let lit = [
                ("fs_picture", "fs_lit_picture"),
                ("fs_texture", "fs_lit_texture"),
                ("fs_text", "fs_lit_text"),
            ];
            let found = lit.iter().filter(|(e, _)| entry(layer) == Some(e) || (styled && *e == "fs_text"));
            found.for_each(|(_, l)| pairs.extend([(*l, m), (*l, Mode::Normal)]));
        }
        if let Kind::Group { layers, clip, .. } = &layer.kind {
            walk(layers, *clip, pairs);
        }
    }
}

impl Pipes {
    pub(crate) fn warm(&self, device: &wgpu::Device, scene: &Scene, format: Format, srgb: bool) {
        let mut pairs = BTreeSet::from([("fs_texture", Mode::Normal), ("fs_texture", Mode::Replace)]);
        walk(&scene.layers, false, &mut pairs);
        scene.templates.values().for_each(|t| walk(std::slice::from_ref(t), true, &mut pairs));
        for (e, m) in pairs {
            let shader = if e.starts_with("fs_lit") { Shader::Lit(e) } else { Shader::Scene(e) };
            let _ = self.get(device, shader, m, format);
        }
        if srgb {
            let _ = self.get(device, Shader::Scene("fs_texture"), Mode::Replace, Format::Rgba8UnormSrgb);
        }
        if scene.drawable().any(|l| matches!(&l.kind, Kind::Text(t) if t.styled())) {
            drop(self.fielders(device));
        }
        if scene.drawable().any(|l| matches!(l.kind, Kind::Image { .. })) {
            let linear = scene.assets.values().any(|a| a.linear);
            let formats = [Some(Format::Rgba8UnormSrgb), linear.then_some(Format::Rgba8Unorm)];
            formats.into_iter().flatten().for_each(|f| drop(self.shrinker(device, f)));
        }
    }
}
