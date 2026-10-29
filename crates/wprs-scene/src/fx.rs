use crate::effects::{Descriptor, Fuse, Output, Pace, ParamDef, ParamKind, PassDef, SLOTS, Solve, Use};
use crate::value::Param;
use crate::{Blend, Class, Color, MAX_PARAMS, Quality};

impl ParamKind {
    pub fn asset(self) -> bool {
        matches!(self, ParamKind::Texture | ParamKind::Lut | ParamKind::Mask)
    }
}

impl ParamDef {
    pub fn accepts(&self, value: &Param) -> bool {
        match (self.kind, value) {
            (ParamKind::Int, Param::Number(n)) => n.fract() == 0.0,
            (ParamKind::Enum, Param::Number(n)) => self.values.iter().any(|e| e.value == *n),
            (ParamKind::Blend, Param::Text(name)) => Blend::named(name).is_some(),
            (kind, value) => matches!(
                (kind, value),
                (ParamKind::Float, Param::Number(_))
                    | (ParamKind::Vec2 | ParamKind::Position, Param::Vec2(_) | Param::Number(_))
                    | (ParamKind::Color, Param::Color(_))
                    | (ParamKind::Bool, Param::Bool(_))
                    | (ParamKind::Texture | ParamKind::Lut | ParamKind::Mask, Param::Text(_))
            ),
        }
    }

    pub fn named<'a>(&'a self, e: &'a crate::EffectUse) -> Option<&'a str> {
        match (e.params.get(&self.id), &self.default) {
            (Some(crate::Value::Fixed(Param::Text(id))), _) | (None, Param::Text(id)) => Some(id),
            _ => None,
        }
    }

    pub fn slot(&self, value: &Param) -> [f32; 4] {
        let clamp = |v: f32| v.clamp(self.min.unwrap_or(f32::MIN), self.max.unwrap_or(f32::MAX));
        match (self.kind, value) {
            (ParamKind::Float | ParamKind::Int | ParamKind::Enum, Param::Number(v)) => {
                [clamp(*v), 0.0, 0.0, 0.0]
            }
            (ParamKind::Vec2 | ParamKind::Position, Param::Number(v)) => [clamp(*v), clamp(*v), 0.0, 0.0],
            (ParamKind::Vec2 | ParamKind::Position, Param::Vec2([x, y])) => [clamp(*x), clamp(*y), 0.0, 0.0],
            (ParamKind::Blend, Param::Text(name)) => {
                [Blend::named(name).map_or(0.0, |b| b as u8 as f32), 0.0, 0.0, 0.0]
            }
            (ParamKind::Color, Param::Color(Color([r, g, b, a]))) => {
                let [r, g, b] = [r, g, b]
                    .map(|c| if *c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) });
                [r, g, b, *a]
            }
            (ParamKind::Bool, Param::Bool(b)) => [f32::from(*b), 0.0, 0.0, 0.0],
            (ParamKind::Texture | ParamKind::Lut | ParamKind::Mask, _) => [0.0; 4],
            _ => self.slot(&self.default),
        }
    }
}

pub(crate) fn live(slot: [f32; 4]) -> bool {
    slot.iter().any(|v| v.abs() > 1e-3)
}

impl Descriptor {
    pub fn parse(json: &str, wgsl: &str) -> Result<Descriptor, serde_json::Error> {
        let mut d: Descriptor = serde_json::from_str(json)?;
        d.wgsl = wgsl.into();
        if d.passes.is_empty() && d.fuse.is_some() {
            let inputs = vec!["previous".into()];
            let entry = format!("fs_{}", d.id);
            let (target, copy, blend, vertex, grid) = Default::default();
            d.passes.push(PassDef {
                entry,
                scale: 1.0,
                inputs,
                output: Output::Next,
                target,
                copy,
                blend,
                vertex,
                grid,
            });
        }
        Ok(d)
    }

    pub fn pack<'a>(&'a self, value: impl Fn(&'a str) -> Option<Param>) -> [[f32; 4]; SLOTS] {
        let mut slots = [[0.0; 4]; SLOTS];
        for (slot, def) in slots.iter_mut().zip(&self.params) {
            *slot = def.slot(&value(&def.id).unwrap_or_else(|| def.default.clone()));
        }
        if self.solve == Some(Solve::Homography) && self.params.len() + 3 <= SLOTS {
            let rows = homography(std::array::from_fn(|i| [slots[i][0], slots[i][1]]));
            slots[self.params.len()..][..3].copy_from_slice(&rows);
        }
        slots
    }

    pub fn param(&self, id: &str) -> Option<usize> {
        self.params.iter().position(|p| p.id == id)
    }

    pub fn units(&self) -> f32 {
        self.cost.taps * self.cost.passes as f32 * self.cost.scale * self.cost.scale
    }

    pub fn class(&self) -> Class {
        match self.units() {
            u if u <= 1.0 => Class::Light,
            u if u <= 6.0 => Class::Medium,
            _ => Class::Heavy,
        }
    }

    pub fn animates(&self) -> bool {
        !self.uses.is_empty() || self.state.is_some() || self.params.iter().any(|p| p.uses.is_some())
    }

    pub fn needs(&self, input: Use, e: &crate::EffectUse, props: Option<&crate::Values>) -> bool {
        let on = |p: &ParamDef| {
            let value = e.params.get(&p.id).map_or(Some(p.default.clone()), |v| v.settled(props));
            value.is_none_or(|v| live(p.slot(&v)))
        };
        self.uses.contains(&input) || self.params.iter().any(|p| p.uses == Some(input) && on(p))
    }

    pub fn tuned(&self, quality: Quality, id: &str) -> Option<&Param> {
        self.quality.range(quality..).find_map(|(_, t)| t.params.get(id))
    }

    pub fn setting(&self, quality: Quality, id: &str, set: Option<Param>) -> Option<Param> {
        let Some(cap) = self.tuned(quality, id) else { return set };
        let value = set.or_else(|| self.param(id).map(|i| self.params[i].default.clone()));
        Some(match (cap, value) {
            (Param::Number(c), Some(Param::Number(v))) => Param::Number(v.min(*c)),
            _ => cap.clone(),
        })
    }

    pub fn scale_at(&self, quality: Quality) -> f32 {
        self.quality.range(quality..).find_map(|(_, t)| t.scale).unwrap_or(1.0)
    }

    pub fn problems(&self) -> Vec<String> {
        let (id, stateful, mut found) = (&self.id, self.state.is_some(), Vec::new());
        let mut fail = |ok: bool, what: String| {
            if !ok {
                found.push(format!("{id}: {what}"));
            }
        };
        let valid = |s: &str| {
            (1..=32).contains(&s.len()) && s.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_'))
        };
        let unit = |s: f32| s > 0.0 && s <= 1.0;
        let kind = |name: &str, kind| self.param(name).is_some_and(|i| self.params[i].kind == kind);
        fail(valid(id), "the id must match [a-z0-9_]{1,32}".into());
        fail(self.params.len() <= MAX_PARAMS, format!("more than {MAX_PARAMS} parameters"));
        for (i, p) in self.params.iter().enumerate() {
            fail(valid(&p.id) && self.param(&p.id) == Some(i), format!("{} is not a unique id", p.id));
            fail(p.accepts(&p.default), format!("the default of {} does not fit its type", p.id));
            fail(p.min.zip(p.max).is_none_or(|(lo, hi)| lo <= hi), format!("{} has min above max", p.id));
        }
        let layered = self.stage == crate::effects::Stage::Layer;
        fail(
            self.passes.is_empty() != layered,
            "a layer effect needs passes, a material or particle shader has none".into(),
        );
        for (n, pass) in self.passes.iter().enumerate() {
            fail(unit(pass.scale), format!("pass {n} has a scale outside (0, 1]"));
            fail(pass.output == Output::Next || stateful, format!("pass {n} writes a missing state"));
            for input in pass.inputs.iter().chain(&pass.copy) {
                fail(self.input(n, input), format!("pass {n} reads the unknown input {input}"));
            }
        }
        fail(self.reach.as_deref().is_none_or(|r| kind(r, ParamKind::Float)), "reach is no float".into());
        fail(self.min_fps.is_none_or(|f| (1..=240).contains(&f)), "min_fps is outside 1..=240".into());
        fail(
            self.velocity.as_deref().is_none_or(|v| kind(v, ParamKind::Bool)),
            "velocity names no bool".into(),
        );
        let paced = match &self.pace {
            Some(Pace::Rate(id)) => vec![id],
            Some(Pace::Sweep(ids)) => ids.iter().collect(),
            _ => Vec::new(),
        };
        fail(paced.iter().all(|id| kind(id, ParamKind::Float)), "pace names no float".into());
        if let Some(fuse) = self.fuse {
            let name = format!("fn {}_{id}(", if fuse == Fuse::Uv { "uv" } else { "color" });
            let single =
                matches!(self.passes.as_slice(), [p] if p.entry == format!("fs_{id}") && p.scale == 1.0);
            let ok = single && !stateful && self.wgsl.contains(&name);
            fail(ok, format!("a fused effect needs the single pass fs_{id} and {name}"));
        }
        for (level, tune) in &self.quality {
            for (name, value) in &tune.params {
                let fits = self.param(name).is_some_and(|i| self.params[i].accepts(value));
                fail(fits, format!("quality {level:?} sets {name}, which does not fit"));
            }
            fail(tune.scale.is_none_or(unit), format!("quality {level:?} has a scale outside (0, 1]"));
        }
        fail(self.state.is_none_or(|s| unit(s.scale)), "the state has a scale outside (0, 1]".into());
        let corners = self.params.len() + 3 <= SLOTS
            && self.fuse.is_none()
            && self.params.iter().take(4).filter(|p| p.kind == ParamKind::Vec2).count() == 4;
        fail(
            self.solve.is_none() || corners,
            "a homography needs four vec2 corners, three free slots and no fuse".into(),
        );
        found.extend(self.extended().into_iter().map(|p| format!("{id}: {p}")));
        found
    }

    pub(crate) fn input(&self, n: usize, input: &str) -> bool {
        let kind = |name: &str, kind| self.param(name).is_some_and(|i| self.params[i].kind == kind);
        match input.split_once(':') {
            Some(("pass", k)) => k.parse::<usize>().is_ok_and(|k| k < n),
            Some(("asset", p)) => kind(p, ParamKind::Texture) || kind(p, ParamKind::Mask),
            Some(("target", t)) => self.targets.contains_key(t),
            Some(_) => false,
            None => {
                ["previous", "input", "source", "mask", "backdrop"].contains(&input)
                    || (input == "state" && self.state.is_some())
            }
        }
    }
}

fn homography([p0, p1, p2, p3]: [[f32; 2]; 4]) -> [[f32; 4]; 3] {
    let (dx, dy) = ([p1[0] - p2[0], p3[0] - p2[0]], [p1[1] - p2[1], p3[1] - p2[1]]);
    let sum = [0, 1].map(|i| p0[i] - p1[i] + p2[i] - p3[i]);
    let den = dx[0] * dy[1] - dx[1] * dy[0];
    let (g, h) = match den.abs() > 1e-9 {
        true => ((sum[0] * dy[1] - dx[1] * sum[1]) / den, (dx[0] * sum[1] - sum[0] * dy[0]) / den),
        false => (0.0, 0.0),
    };
    let [a, d] = [0, 1].map(|i| p1[i] - p0[i] + g * p1[i]);
    let [b, e] = [0, 1].map(|i| p3[i] - p0[i] + h * p3[i]);
    let [c, f] = p0;
    [
        [e - f * h, c * h - b, b * f - c * e, 0.0],
        [f * g - d, a - c * g, c * d - a * f, 0.0],
        [d * h - e * g, b * g - a * h, a * e - b * d, 0.0],
    ]
}
