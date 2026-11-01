use crate::effects::{Descriptor, Fuse, Output, Pace, ParamDef, ParamKind, PassDef, SLOTS, Solve, Use};
use crate::value::Param;
use crate::{Class, MAX_PARAMS};
use homography::homography;
use slots::live;
use std::borrow::Cow;

mod homography;
pub(crate) mod slots;
mod tune;

const READS: f32 = 4.0;
const OPS: f32 = 512.0;

impl Descriptor {
    pub fn parse(json: &str, wgsl: &str) -> Result<Descriptor, serde_json::Error> {
        let mut d: Descriptor = serde_json::from_str(json)?;
        d.wgsl = wgsl.into();
        if d.passes.is_empty() && d.fuse.is_some() {
            let inputs = vec!["previous".into()];
            let entry = format!("fs_{}", d.id);
            let (target, copy, blend, vertex, grid, swap) = Default::default();
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
                swap,
            });
        }
        Ok(d)
    }

    pub fn pack<'a>(&'a self, value: impl Fn(&'a str) -> Option<Cow<'a, Param>>) -> [[f32; 4]; MAX_PARAMS] {
        let mut slots = [[0.0; 4]; MAX_PARAMS];
        let pictures =
            |def: &&ParamDef| matches!(def.kind, ParamKind::Texture | ParamKind::Lut | ParamKind::Mask);
        for (slot, def) in slots.iter_mut().zip(&self.params).filter(|(_, def)| !pictures(def)) {
            *slot = def.slot(value(&def.id).as_deref().unwrap_or(&def.default));
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
        let declared = self.cost.taps * self.cost.passes as f32 * self.cost.scale * self.cost.scale;
        let side =
            |p: &PassDef| p.target.as_ref().and_then(|t| self.targets.get(t)).map_or(p.scale, |t| t.scale);
        let drawn = self.passes.iter().filter(|p| p.swap.is_none());
        let floor = drawn.map(|p| side(p).powi(2) * crate::measure::spread(p)).sum::<f32>();
        if self.key == 0 { declared } else { declared.max(floor).max(self.reads / READS).max(self.ops / OPS) }
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
        fail(
            self.state.as_ref().is_none_or(|s| unit(s.scale)),
            "the state has a scale outside (0, 1]".into(),
        );
        let fade = self.state.as_ref().and_then(|s| s.fade.as_deref());
        fail(fade.is_none_or(|f| kind(f, ParamKind::Float)), "the state fades by no float".into());
        let last = self.passes.iter().rposition(|p| p.output == Output::State);
        let mut steps = self.passes.iter().enumerate().filter(|(_, p)| p.output == Output::Step);
        fail(
            steps.all(|(n, _)| last.is_some_and(|l| n < l)),
            "a step pass comes after the last state pass".into(),
        );
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
