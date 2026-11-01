use crate::Kind;
use crate::probe::Probe;
use crate::validate::{Check, Issue};
use crate::value::Vec2;

impl Check<'_> {
    pub(super) fn kind(&mut self, p: &str, kind: &Kind, depth: usize) {
        match kind {
            Kind::Group { layers, color, .. } => {
                self.tint(p, color.as_ref());
                self.layers(&format!("{p}/layers"), layers, depth + 1)
            }
            Kind::Image { image: named, size, color, playback }
            | Kind::Video { video: named, size, color, playback } => {
                let field = if matches!(kind, Kind::Image { .. }) { "image" } else { "video" };
                let pooled = matches!(named, crate::Value::Fixed(id) if field == "video" && self.scene.pools.contains_key(id));
                match pooled {
                    true => self.uses(true, "script_geometry"),
                    false => self.named(&format!("{p}/{field}"), named),
                }
                self.size(&format!("{p}/size"), *size);
                self.tint(p, color.as_ref());
                playback.iter().for_each(|b| self.playback(&format!("{p}/playback"), b));
            }
            Kind::Text(t) => self.text(p, t),
            Kind::Shape { shape, fill, gradient, stroke } => {
                self.shape(&format!("{p}/shape"), shape);
                fill.iter().for_each(|f| self.value(&format!("{p}/fill"), f));
                if let Some(g) = gradient {
                    self.range(&format!("{p}/gradient/stops"), g.stops.len() as f32, 2.0, 4.0);
                    self.keys_sorted(&format!("{p}/gradient/stops"), g.stops.iter().map(|s| s.0));
                    Vec2::probe(self, &format!("{p}/gradient/from"), &Vec2(g.from));
                    Vec2::probe(self, &format!("{p}/gradient/to"), &Vec2(g.to));
                    for (i, (_, color)) in g.stops.iter().enumerate() {
                        self.color(&format!("{p}/gradient/stops/{i}/1"), *color);
                    }
                }
                if let Some(s) = stroke {
                    self.value(&format!("{p}/stroke/color"), &s.color);
                    self.range(&format!("{p}/stroke/width"), s.width, 0.0, 1e4);
                }
            }
            Kind::Particles { system, color, instance, render } => {
                let known = self.scene.particles.get(system);
                self.known(&format!("{p}/system"), "particle system", system, known.is_some());
                self.tint(p, color.as_ref());
                if let Some(r) = render {
                    self.uses(true, "particles2");
                    self.render(p, r);
                }
                if let Some(i) = instance {
                    self.instance(&format!("{p}/instance"), i, known);
                }
            }
            Kind::Spectrum(s) => {
                if ![16, 32, 64].contains(&s.bands) {
                    self.issues.push(Issue::Range {
                        path: format!("{p}/bands"),
                        value: s.bands as f32,
                        min: 16.0,
                        max: 64.0,
                    });
                }
                self.size(&format!("{p}/size"), Some(s.size));
                self.range(&format!("{p}/gap"), s.gap, 0.0, 0.95);
                self.value(&format!("{p}/color"), &s.color);
            }
            Kind::Solid { color, size } => {
                self.value(&format!("{p}/color"), color);
                self.size(&format!("{p}/size"), *size);
            }
            Kind::Adjustment { size, redraw, .. } => {
                let [w, h] = self.scene.canvas.size.map(|v| 2.0 * v as f32);
                if let Some([x, y]) = size {
                    self.range(&format!("{p}/size/0"), *x, 1.0, w);
                    self.range(&format!("{p}/size/1"), *y, 1.0, h);
                }
                self.uses(size.is_some() || *redraw, "capture");
            }
            Kind::Sound(_) | Kind::Light(_) | Kind::Mesh(_) | Kind::Model(_) => {}
        }
    }

    pub(super) fn size(&mut self, path: &str, size: Option<[f32; 2]>) {
        size.iter()
            .flatten()
            .enumerate()
            .for_each(|(i, v)| self.range(&format!("{path}/{i}"), *v, 1e-3, 1e5));
    }
}
