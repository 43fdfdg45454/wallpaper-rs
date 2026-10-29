use crate::effects::{Descriptor, ParamKind, SLOTS};
use crate::shader::{bindings, compile, reads};
use crate::value::Param;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub const CONTRACT: &str = concat!(
    include_str!("frame.wgsl"),
    include_str!("prelude.wgsl"),
    include_str!("helpers.wgsl"),
    include_str!("relief.wgsl"),
    include_str!("contract.wgsl"),
    include_str!("blend.wgsl"),
    include_str!("stages.wgsl")
);
pub const MAX_EFFECT_FILES: usize = 32;
pub const MAX_PARAMS: usize = 24;
pub const MAX_INPUTS: usize = 9;
pub const MAX_TARGETS: usize = 8;
pub const MAX_OPTIONS: usize = 8;
pub const MAX_VARIANTS: usize = 8;
pub const MAX_GRID: u32 = 64;
pub const MAX_WGSL: usize = 64 << 10;
pub const MAX_WGSL_TOTAL: usize = 256 << 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EffectFiles {
    pub descriptor: String,
    pub wgsl: String,
}

impl Descriptor {
    pub fn packaged(id: &str, json: &str, wgsl: &str) -> Result<Descriptor, String> {
        let raw: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let mut d = Descriptor::parse(json, wgsl).map_err(|e| e.to_string())?;
        let taken = crate::reserved(&d.id);
        let checks = [
            (!taken, format!("{} is the name of a built-in effect", d.id)),
            (d.id == id, format!("declares the id {}, expected {id}", d.id)),
            (raw.get("cost").is_some(), "needs a cost".into()),
            (wgsl.len() <= MAX_WGSL, format!("its shader is larger than {} KiB", MAX_WGSL >> 10)),
        ];
        if let Some((_, problem)) = checks.into_iter().find(|(ok, _)| !ok) {
            return Err(problem);
        }
        if let Some(problem) = d.problems().into_iter().next() {
            return Err(problem);
        }
        let source = format!("{CONTRACT}\n{wgsl}");
        let module = compile(&source)?;
        let entries: Vec<&str> =
            d.passes.iter().flat_map(|p| [p.entry.as_str()].into_iter().chain(p.vertex.as_deref())).collect();
        let mut entries: Vec<&str> = entries.into_iter().filter(|e| !e.is_empty()).collect();
        if let Some(missing) = entries.iter().find(|e| !module.entry_points.iter().any(|p| p.name == **e)) {
            return Err(format!("its shader has no entry point {missing}"));
        }
        if let Some((name, kind)) = d.stage.function() {
            let typed = |h: naga::Handle<naga::Type>| module.types[h].name.as_deref() == Some(kind);
            let found = module.functions.iter().any(|(_, f)| {
                let args = matches!(f.arguments.as_slice(), [a] if typed(a.ty));
                f.name.as_deref() == Some(name) && args && f.result.as_ref().is_some_and(|r| typed(r.ty))
            });
            if !found {
                return Err(format!("its shader needs fn {name}(value: {kind}) -> {kind}"));
            }
            entries.push(name);
        }
        if let Some(o) =
            d.options.iter().find(|o| !module.overrides.iter().any(|(_, v)| v.name.as_deref() == Some(&o.id)))
        {
            return Err(format!("its shader declares no override {}", o.id));
        }
        static ALLOWED: OnceLock<usize> = OnceLock::new();
        let allowed = *ALLOWED.get_or_init(|| compile(CONTRACT).map_or(0, |m| bindings(&m)));
        if bindings(&module) > allowed {
            return Err("its shader declares bindings of its own".into());
        }
        for found in reads(&module, &entries) {
            if !d.uses.contains(&found) {
                d.uses.push(found);
            }
        }
        Ok(d)
    }

    pub fn newer(&self) -> bool {
        let params = self.params.iter().any(|p| {
            let kind = !matches!(
                p.kind,
                ParamKind::Float
                    | ParamKind::Vec2
                    | ParamKind::Color
                    | ParamKind::Bool
                    | ParamKind::Texture
                    | ParamKind::Lut
            );
            kind || !p.label.is_empty()
                || !p.help.is_empty()
                || !p.values.is_empty()
                || p.channels.is_some()
                || p.paint.is_some()
        });
        let passes = self.passes.iter().any(|p| {
            let fields = p.target.is_some()
                || p.copy.is_some()
                || p.blend.is_some()
                || p.vertex.is_some()
                || p.grid.is_some();
            let inputs = p.inputs.iter().any(|i| i == "backdrop" || i.starts_with("target:"));
            fields || inputs || p.inputs.len() > 5
        });
        let more = !self.targets.is_empty() || !self.options.is_empty() || !self.stage.is_layer();
        params || passes || self.params.len() > SLOTS || more
    }

    pub(crate) fn extended(&self) -> Vec<String> {
        let mut found = Vec::new();
        let mut fail = |ok: bool, what: String| {
            if !ok {
                found.push(what);
            }
        };
        let valid = |s: &str| {
            (1..=32).contains(&s.len()) && s.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_'))
        };
        let texts = self.params.iter().flat_map(|p| p.label.values().chain(p.help.values()));
        let texts =
            texts.chain(self.params.iter().flat_map(|p| p.values.iter().flat_map(|e| e.label.values())));
        fail(
            texts.chain(self.name.values()).all(|t| t.chars().count() <= 256),
            "a label or help is longer than 256 characters".into(),
        );
        for p in &self.params {
            let id = &p.id;
            let listed = p.kind != ParamKind::Enum || (1..=64).contains(&p.values.len());
            fail(listed, format!("{id} needs 1 to 64 values"));
            fail(
                p.kind == ParamKind::Enum || p.values.is_empty(),
                format!("{id} lists values but is no enum"),
            );
            let channels = p.channels.is_none_or(|c| p.kind == ParamKind::Mask && [1, 3, 4].contains(&c));
            fail(channels, format!("{id} has channels other than 1, 3 or 4, or is no mask"));
            let paint = p
                .paint
                .as_ref()
                .is_none_or(|v| p.kind == ParamKind::Mask && matches!(v, Param::Color(_) | Param::Number(_)));
            fail(paint, format!("{id} paints with something other than a color or a number, or is no mask"));
        }
        fail(self.targets.len() <= MAX_TARGETS, format!("more than {MAX_TARGETS} targets"));
        for (name, target) in &self.targets {
            fail(valid(name), format!("the target {name} must match [a-z0-9_]{{1,32}}"));
            fail(
                target.scale > 0.0 && target.scale <= 2.0,
                format!("the target {name} has a scale outside (0, 2]"),
            );
        }
        for (n, pass) in self.passes.iter().enumerate() {
            fail(
                pass.copy.is_some() != !pass.entry.is_empty(),
                format!("pass {n} needs either an entry or a copy"),
            );
            fail(pass.inputs.len() <= MAX_INPUTS, format!("pass {n} reads more than {MAX_INPUTS} inputs"));
            let written = pass.target.as_ref().is_none_or(|t| self.targets.contains_key(t));
            fail(written, format!("pass {n} writes an unknown target"));
            fail(
                pass.target.is_none() || pass.output == crate::Output::Next,
                format!("pass {n} writes both a target and the state"),
            );
            fail(
                pass.vertex.as_ref().is_none_or(|v| !v.is_empty()),
                format!("pass {n} has an empty vertex entry"),
            );
            let grid = pass
                .grid
                .is_none_or(|g| pass.vertex.is_some() && g.iter().all(|c| (1..=MAX_GRID).contains(c)));
            fail(grid, format!("pass {n} needs a vertex entry and a grid of 1 to {MAX_GRID} cells a side"));
        }
        fail(self.options.len() <= MAX_OPTIONS, format!("more than {MAX_OPTIONS} options"));
        for (i, o) in self.options.iter().enumerate() {
            let id = &o.id;
            let unique =
                self.param(id).is_none() && self.options.iter().position(|other| &other.id == id) == Some(i);
            fail(valid(id) && unique, format!("the option {id} is not a unique id"));
            let fits =
                |v: &Param| matches!(v, Param::Bool(_)) || matches!(v, Param::Number(n) if n.fract() == 0.0);
            fail(
                (1..=16).contains(&o.values.len()) && o.values.iter().all(fits),
                format!("the option {id} needs 1 to 16 whole numbers or flags"),
            );
            fail(
                o.values.contains(&o.default),
                format!("the default of the option {id} is not one of its values"),
            );
            let bound = o
                .when_bound
                .as_ref()
                .is_none_or(|t| self.param(t).is_some_and(|i| self.params[i].kind.asset()));
            fail(bound, format!("the option {id} waits for a parameter that is no texture"));
        }
        found
    }
}
