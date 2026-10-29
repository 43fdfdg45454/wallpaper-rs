use crate::light::{Beam, Light, MAX_LIGHTS};
use crate::typeset::{MAX_FALLBACK, TextLayer};
use crate::validate::{Check, Issue};
use crate::{Kind, Layer};

impl Check<'_> {
    pub(crate) fn text(&mut self, p: &str, t: &TextLayer) {
        self.value(&format!("{p}/text"), &t.text);
        self.value(&format!("{p}/color"), &t.color);
        self.range(&format!("{p}/size"), t.size, 1.0, 1024.0);
        self.count(&format!("{p}/fallback"), t.fallback.len(), MAX_FALLBACK);
        t.fallback.iter().enumerate().for_each(|(i, f)| self.filled(&format!("{p}/fallback/{i}"), f));
        self.uses(!t.fallback.is_empty(), "fonts2");
        self.uses(t.laid_out(), "text_layout");
        self.uses(t.styled(), "text_styles");
        if let Some(w) = &t.wrap {
            self.range(&format!("{p}/wrap/width"), w.width, 1.0, 16_384.0);
            w.lines.iter().for_each(|n| self.range(&format!("{p}/wrap/lines"), *n as f32, 1.0, 1000.0));
        }
        t.line_height.iter().for_each(|h| self.range(&format!("{p}/line_height"), *h, 0.5, 4.0));
        t.padding.iter().for_each(|v| self.range(&format!("{p}/padding"), *v, 0.0, 1024.0));
        if let Some(b) = &t.background {
            self.value(&format!("{p}/background/color"), &b.color);
            let sizes =
                b.padding.iter().enumerate().map(|(i, v)| (format!("{p}/background/padding/{i}"), *v));
            for (path, v) in sizes.chain([(format!("{p}/background/radius"), b.radius)]) {
                self.range(&path, v, 0.0, 1024.0);
            }
        }
        t.letter_spacing.iter().for_each(|v| self.number(&format!("{p}/letter_spacing"), v, -100.0, 1000.0));
        if let Some(o) = &t.outline {
            self.value(&format!("{p}/outline/color"), &o.color);
            self.number(&format!("{p}/outline/width"), &o.width, 0.0, 64.0);
        }
        if let Some(s) = &t.shadow {
            self.value(&format!("{p}/shadow/color"), &s.color);
            self.value(&format!("{p}/shadow/offset"), &s.offset);
            self.number(&format!("{p}/shadow/blur"), &s.blur, 0.0, 64.0);
        }
        t.blur.iter().for_each(|v| self.number(&format!("{p}/blur"), v, 0.0, 64.0));
    }

    pub(crate) fn light(&mut self, p: &str, layer: &Layer, l: &Light) {
        self.uses(true, "lights");
        let mut bare = Layer::new(layer.id.clone(), layer.kind.clone());
        (bare.transform, bare.anchor, bare.visible) =
            (layer.transform.clone(), layer.anchor, layer.visible.clone());
        (bare.parent, bare.inherit) = (layer.parent.clone(), layer.inherit.clone());
        bare.attach = layer.attach.clone();
        if *layer != bare {
            self.issues.push(Issue::Light(p.into()));
        }
        self.value(&format!("{p}/color"), &l.color);
        self.number(&format!("{p}/intensity"), &l.intensity, 0.0, 100.0);
        self.number(&format!("{p}/radius"), &l.radius, 1.0, 1e5);
        self.number(&format!("{p}/z"), &l.z, -1e5, 1e5);
        let only = |on: bool, beams: &[Beam]| on && !beams.contains(&l.light);
        let misplaced = [
            ("cone", only(l.cone.is_some(), &[Beam::Spot])),
            ("length", only(l.length.is_some(), &[Beam::Tube])),
            ("volumetric", only(l.volumetric.is_some(), &[Beam::Point, Beam::Spot])),
            ("texture", only(l.texture.is_some(), &[Beam::Spot])),
            ("shadows", only(l.shadows, &[Beam::Point, Beam::Spot, Beam::Directional])),
        ];
        self.uses(l.shadows, "shadows");
        for (name, _) in misplaced.into_iter().filter(|(_, wrong)| *wrong) {
            self.issues.push(Issue::Type(format!("{p}/{name}")));
        }
        if let Some([inner, outer]) = l.cone {
            self.range(&format!("{p}/cone/0"), inner, 0.0, 180.0);
            self.range(&format!("{p}/cone/1"), outer, inner, 180.0);
        }
        l.length.iter().for_each(|v| self.range(&format!("{p}/length"), *v, 1.0, 1e5));
        if let Some(v) = l.volumetric {
            self.uses(true, "volumetric_lights");
            self.range(&format!("{p}/volumetric/density"), v.density, 0.0, 1.0);
            self.range(&format!("{p}/volumetric/samples"), v.samples as f32, 4.0, 64.0);
        }
        if let Some(texture) = &l.texture {
            self.uses(true, "light_textures");
            self.named(&format!("{p}/texture"), texture);
            self.moving(texture);
        }
    }

    pub(crate) fn lights(&mut self) {
        let count = self.scene.layers().filter(|l| matches!(l.kind, Kind::Light(_))).count();
        if count > MAX_LIGHTS {
            self.issues.push(Issue::Lights(count));
        }
        let shadows = self.scene.layers().filter(|l| matches!(&l.kind, Kind::Light(l) if l.shadows)).count();
        if shadows > crate::figure::MAX_SHADOWS {
            self.issues.push(Issue::Shadows(shadows));
        }
    }
}
