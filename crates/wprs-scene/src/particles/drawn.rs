use super::Render;
use crate::effects::Stage;
use crate::validate::{Check, Issue};

impl Check<'_> {
    pub(super) fn render2(&mut self, p: &str, r: &Render) {
        let wide = r.brightness.is_some()
            || r.order.is_some()
            || r.cycles.is_some()
            || r.stretch.min.is_some()
            || r.stretch.max.is_some();
        self.uses(wide, "particles2");
        r.brightness.iter().for_each(|b| self.number(&format!("{p}/brightness"), b, 0.0, 16.0));
        if let Some(n) = r.cycles {
            self.range(&format!("{p}/cycles"), n as f32, 1.0, 64.0);
            if r.frames != Some(super::Frames::Life) {
                self.issues.push(Issue::Type(format!("{p}/cycles")));
            }
        }
        let [lo, hi] = [r.stretch.min.unwrap_or(0.0), r.stretch.max.unwrap_or(1e4)];
        self.range(&format!("{p}/stretch/min"), lo, 0.0, 1e4);
        self.range(&format!("{p}/stretch/max"), hi, lo, 1e4);
        if let Some([start, end, opacity]) = r.cutout {
            self.uses(true, "particle_looks");
            self.range(&format!("{p}/cutout/0"), start, 0.0, 1.0);
            self.range(&format!("{p}/cutout/1"), end, start.next_up(), 1.0);
            self.range(&format!("{p}/cutout/2"), opacity, 0.0, 1.0);
        }
        self.uses(r.orient.is_some() || r.angle.is_some(), "particle_looks");
        r.angle.iter().for_each(|a| self.range(&format!("{p}/angle"), *a, -360.0, 360.0));
        if r.angle.is_some() && r.orient != Some(super::Orient::Fixed) {
            self.issues.push(Issue::Type(format!("{p}/angle")));
        }
        if let Some(rope) = r.rope {
            self.uses(true, "particle_ropes");
            self.range(&format!("{p}/rope/subdivide"), rope.subdivide as f32, 0.0, 7.0);
            self.range(&format!("{p}/rope/width"), rope.width, 0.0, 1e3);
            self.range(&format!("{p}/rope/uv_scale"), rope.uv_scale, 0.0, 100.0);
            self.range(&format!("{p}/rope/uv_scroll"), rope.uv_scroll, -100.0, 100.0);
        }
        if let Some(t) = &r.trail {
            let more =
                [t.segments.is_some(), t.subdivide.is_some(), t.uv_scale.is_some(), t.uv_scroll.is_some()];
            self.uses(t.length > 16 || more.contains(&true) || t.textured, "particle_ropes");
            t.segments.iter().for_each(|n| self.range(&format!("{p}/trail/segments"), *n as f32, 1.0, 64.0));
            t.subdivide.iter().for_each(|n| self.range(&format!("{p}/trail/subdivide"), *n as f32, 0.0, 7.0));
            t.uv_scale.iter().for_each(|v| self.range(&format!("{p}/trail/uv_scale"), *v, 0.0, 100.0));
            t.uv_scroll.iter().for_each(|v| self.range(&format!("{p}/trail/uv_scroll"), *v, -100.0, 100.0));
        }
        self.uses(r.lit || r.normal.is_some() || r.refract.is_some(), "particle_lighting");
        r.normal.iter().for_each(|n| self.picture(&format!("{p}/normal"), n));
        if let Some(f) = &r.refract {
            self.picture(&format!("{p}/refract/normal"), &f.normal);
            self.range(&format!("{p}/refract/amount"), f.amount, 0.0, 1.0);
        }
        if let Some(id) = &r.shader {
            self.uses(true, "particle_shaders");
            self.staged(&format!("{p}/shader"), id, Stage::Particle);
        }
    }

    pub(crate) fn staged(&mut self, path: &str, id: &str, stage: Stage) {
        let scene = self.scene;
        let declared = scene.effects.contains_key(id);
        self.known(path, "shader", id, declared);
        if scene.loaded.get(id).is_some_and(|d| d.stage != stage) {
            self.issues.push(Issue::Type(path.into()));
        }
    }
}
