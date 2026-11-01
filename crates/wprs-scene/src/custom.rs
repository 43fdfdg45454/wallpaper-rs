use crate::effects::{Descriptor, ParamKind, SLOTS};
use crate::value::Param;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONTRACT: &str = concat!(
    include_str!("frame.wgsl"),
    include_str!("prelude.wgsl"),
    include_str!("helpers.wgsl"),
    include_str!("oklab.wgsl"),
    include_str!("noise.wgsl"),
    include_str!("dither.wgsl"),
    include_str!("relief.wgsl"),
    include_str!("blend.wgsl"),
    include_str!("contract.wgsl"),
    include_str!("cell.wgsl"),
    include_str!("stages.wgsl")
);
pub const MAX_EFFECT_FILES: usize = 32;
pub const MAX_PARAMS: usize = 24;
pub const MAX_INPUTS: usize = 9;
pub const MAX_TARGETS: usize = 8;
pub const MAX_OPTIONS: usize = 8;
pub const MAX_VARIANTS: usize = 8;
pub const MAX_GRID: u32 = 64;
pub const MAX_PASSES: usize = 16;
pub const MAX_WGSL: usize = 64 << 10;
pub const MAX_WGSL_TOTAL: usize = 256 << 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EffectFiles {
    pub descriptor: String,
    pub wgsl: String,
}

impl Descriptor {
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
                || p.yields_to.is_some()
        });
        let passes = self.passes.iter().any(|p| {
            let fields = p.target.is_some()
                || p.copy.is_some()
                || p.swap.is_some()
                || p.blend.is_some()
                || p.vertex.is_some()
                || p.grid.is_some();
            let inputs = p.inputs.iter().any(|i| i == "backdrop" || i.starts_with("target:"));
            fields || inputs || p.inputs.len() > 5
        });
        let steps = matches!(self.pace, Some(crate::Pace::Steps(_)));
        let more = !self.targets.is_empty()
            || !self.options.is_empty()
            || !self.stage.is_layer()
            || self.compose
            || steps;
        params || passes || self.params.len() > SLOTS || more
    }

    pub fn newest(&self) -> bool {
        const HELPERS: [&str; 12] = [
            "mask_rgb",
            "outside_layer",
            "OUTSIDE_UV",
            "time_phase",
            "layer_extent",
            "cursor_last",
            "grid_uv",
            "grid_position",
            "fs_copy",
            "bands_of",
            "band_left_of",
            "band_right_of",
        ];
        let mut words = self.wgsl.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'));
        let fields = self.compose || self.min_fps.is_some() || self.velocity.is_some();
        let fields =
            fields || self.passes.iter().any(|p| p.swap.is_some()) || self.wgsl.contains("more.grid");
        fields || words.any(|w| HELPERS.contains(&w))
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
            let yields = p.yields_to.as_ref().is_none_or(|o| o != id && self.param(o).is_some());
            fail(yields, format!("{id} yields to no other parameter of the effect"));
        }
        let c = self.cost;
        fail(
            [c.taps, c.scale].iter().all(|v| v.is_finite() && *v >= 0.0) && c.passes <= MAX_PASSES as u32,
            format!("the cost needs finite numbers of at least 0 and at most {MAX_PASSES} passes"),
        );
        fail(self.targets.len() <= MAX_TARGETS, format!("more than {MAX_TARGETS} targets"));
        for (name, target) in &self.targets {
            fail(valid(name), format!("the target {name} must match [a-z0-9_]{{1,32}}"));
            fail(
                target.scale > 0.0 && target.scale <= 2.0,
                format!("the target {name} has a scale outside (0, 2]"),
            );
        }
        fail(self.passes.len() <= MAX_PASSES, format!("more than {MAX_PASSES} passes"));
        for (n, pass) in self.passes.iter().enumerate() {
            let kinds = [!pass.entry.is_empty(), pass.copy.is_some(), pass.swap.is_some()];
            fail(
                kinds.iter().filter(|k| **k).count() == 1,
                format!("pass {n} needs either an entry, a copy or a swap"),
            );
            if let Some([a, b]) = &pass.swap {
                let same = self.targets.get(a).zip(self.targets.get(b)).is_some_and(|(x, y)| x == y);
                fail(
                    a != b && same,
                    format!("pass {n} swaps targets that are not two of the same size and format"),
                );
            }
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
