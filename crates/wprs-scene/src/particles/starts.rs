use super::seed::{MAX_PALETTE, MAX_REMAPS, REMAP_INPUTS, Remap};
use super::{Handle, Init, System, Target};
use crate::validate::{Check, Issue};
use crate::value::{Band, Level};

impl Check<'_> {
    pub(super) fn init2(&mut self, p: &str, i: &Init, s: &System) {
        self.exponent(&format!("{p}/color"), i.color.exponent);
        if let Some(a) = &i.alpha {
            self.uses(true, "particles2");
            self.spread(&format!("{p}/alpha"), a, 0.0, 1.0);
        }
        let colored = [i.hsv.is_some(), i.palette.is_some(), !i.gradient.is_empty()];
        if colored.iter().filter(|c| **c).count() > 1 {
            self.issues.push(Issue::Type(format!("{p}/palette")));
        }
        if !i.gradient.is_empty() {
            self.uses(true, "particles2");
            let q = format!("{p}/gradient");
            self.count(&q, i.gradient.len(), super::MAX_STOPS);
            self.keys_sorted(&q, i.gradient.iter().map(|k| k.0));
            for (k, (t, c)) in i.gradient.iter().enumerate() {
                self.range(&format!("{q}/{k}/0"), *t, 0.0, 1.0);
                self.color(&format!("{q}/{k}/1"), *c);
            }
        }
        if let Some(h) = &i.hsv {
            self.uses(true, "particles2");
            for (name, [lo, hi], max) in [("hue", h.hue, 360.0), ("sat", h.sat, 1.0), ("val", h.val, 1.0)] {
                self.range(&format!("{p}/hsv/{name}/0"), lo, 0.0, max);
                self.range(&format!("{p}/hsv/{name}/1"), hi, lo, max);
            }
            h.steps.iter().for_each(|n| self.range(&format!("{p}/hsv/steps"), *n as f32, 1.0, 360.0));
        }
        if let Some(pal) = &i.palette {
            self.uses(true, "particles2");
            self.range(&format!("{p}/palette/colors"), pal.colors.len() as f32, 1.0, MAX_PALETTE as f32);
            pal.colors
                .iter()
                .enumerate()
                .for_each(|(k, c)| self.color(&format!("{p}/palette/colors/{k}"), *c));
            pal.noise
                .iter()
                .enumerate()
                .for_each(|(k, v)| self.range(&format!("{p}/palette/noise/{k}"), *v, 0.0, 1.0));
        }
        if let Some(v) = &i.velocity {
            self.uses(true, "particles2");
            for (k, [x, y]) in v.ends.iter().enumerate() {
                self.range(&format!("{p}/velocity/{k}/0"), *x, -1e5, 1e5);
                self.range(&format!("{p}/velocity/{k}/1"), *y, -1e5, 1e5);
            }
            self.exponent(&format!("{p}/velocity"), v.exponent);
        }
        if let Some(r) = &i.radial {
            self.uses(true, "particles2");
            self.spread(&format!("{p}/radial"), r, -1e5, 1e5);
        }
        if let Some(c) = &i.inherit {
            self.uses(true, "particles2");
            if matches!(c.from, Target::Point(_)) {
                self.issues.push(Issue::Type(format!("{p}/inherit/from")));
            }
            self.target(&format!("{p}/inherit/from"), &c.from, s);
            self.range(&format!("{p}/inherit/speed/0"), c.speed[0], 0.0, 10.0);
            self.range(&format!("{p}/inherit/speed/1"), c.speed[1], c.speed[0], 10.0);
        }
        if let Some(t) = &i.turbulent {
            self.uses(true, "particle_fields");
            let q = format!("{p}/turbulent");
            self.range(&format!("{q}/forward"), t.forward, -360.0, 360.0);
            self.range(&format!("{q}/scale"), t.scale, 0.0, 1.0);
            self.range(&format!("{q}/noise_speed"), t.noise_speed, 0.0, 100.0);
            self.range(&format!("{q}/speed/1"), t.speed[1], t.speed[0].max(0.0), 1e5);
            [t.offset, t.phase].iter().for_each(|v| self.range(&q, *v, -1e4, 1e4));
            if let Some(Band::Index(n)) = t.audio {
                self.range(&format!("{q}/audio"), n as f32, 0.0, 63.0);
            }
        }
        if let Some(o) = &i.offset_noise {
            self.uses(true, "particle_fields");
            let q = format!("{p}/offset_noise");
            self.range(&format!("{q}/distance"), o.distance, 0.0, 1e5);
            self.range(&format!("{q}/octaves"), o.octaves as f32, 1.0, 6.0);
            for (name, v) in
                [("scale", o.scale), ("noise_scale", o.noise_scale), ("noise_speed", o.noise_speed)]
            {
                self.range(&format!("{q}/{name}"), v, 0.0, 100.0);
            }
            o.sign
                .iter()
                .enumerate()
                .for_each(|(k, v)| self.range(&format!("{q}/sign/{k}"), f32::from(*v), -1.0, 1.0));
        }
        for (name, d) in [("depth", &i.depth), ("depth_speed", &i.depth_speed)] {
            let Some(d) = d else { continue };
            self.uses(true, "particle_depth");
            self.spread(&format!("{p}/{name}"), d, -1e5, 1e5);
            if s.perspective.is_none() {
                self.issues.push(Issue::Type(format!("{p}/{name}")));
            }
        }
        self.count(&format!("{p}/remap"), i.remap.len(), MAX_REMAPS);
        i.remap.iter().enumerate().for_each(|(k, r)| self.remap(&format!("{p}/remap/{k}"), r, s));
    }

    pub(super) fn remap(&mut self, q: &str, r: &Remap, s: &System) {
        self.uses(true, "particle_fields");
        let input = r.input.as_str();
        let point = input.strip_prefix("distance:").map(|h| Handle::try_from(h.to_string()));
        let band = |b: &str| {
            b.parse::<u32>().is_ok_and(|n| n < 64)
                || serde_json::from_value::<Level>(serde_json::Value::String(b.into())).is_ok()
        };
        let known = REMAP_INPUTS.contains(&input)
            || matches!(input, "cursor:x" | "cursor:y")
            || input.strip_prefix("audio:").is_some_and(band)
            || matches!(point, Some(Ok(_)));
        self.known(&format!("{q}/input"), "remap input", input, known);
        if let Some(Ok(h)) = point {
            self.handle(&format!("{q}/input"), h, s);
        }
        let [a, b] = r.from;
        if a == b || !a.is_finite() || !b.is_finite() {
            self.issues.push(Issue::Type(format!("{q}/in")));
        }
        r.to.iter().enumerate().for_each(|(k, v)| self.range(&format!("{q}/out/{k}"), *v, -1e6, 1e6));
    }
}
