use super::{Edit, Kind, Mask, VERSION};
use crate::config::parse_hex_color;
use serde_json::Value;
use std::collections::HashSet;

const PERCENT: (f32, f32) = (-100.0, 100.0);
const UNIT: (f32, f32) = (0.0, 1.0);
const WIDE: (f32, f32) = (-1.0, 2.0);

type Checked = Result<(), String>;

fn within(path: &str, value: f32, (lo, hi): (f32, f32)) -> Checked {
    let fine = (lo..=hi).contains(&value);
    fine.then_some(()).ok_or_else(|| format!("{path}: {value} is out of range {lo} to {hi}"))
}

fn all(path: &str, values: &[f32], range: (f32, f32)) -> Checked {
    values.iter().try_for_each(|v| within(path, *v, range))
}

fn ensure(fine: bool, message: impl FnOnce() -> String) -> Checked {
    fine.then_some(()).ok_or_else(message)
}

fn name(path: &str, text: &str) -> Checked {
    let fine = text.chars().all(|c| c.is_ascii_alphanumeric() || "_-".contains(c));
    ensure((1..=64).contains(&text.len()) && fine, || format!("{path}: use 1 to 64 letters, digits, - or _"))
}

fn rect(path: &str, [x, y, w, h]: [f32; 4]) -> Checked {
    all(path, &[x, y, w, h], UNIT)?;
    ensure(w > 0.0 && h > 0.0 && x + w <= 1.0001 && y + h <= 1.0001, || {
        format!("{path}: not inside the image")
    })
}

fn curve(path: &str, points: &[[f32; 2]]) -> Checked {
    ensure(points.is_empty() || (2..=16).contains(&points.len()), || format!("{path}: use 2 to 16 points"))?;
    all(path, points.as_flattened(), UNIT)?;
    ensure(points.windows(2).all(|p| p[0][0] < p[1][0]), || {
        format!("{path}: x must grow from point to point")
    })
}

fn unknown(given: &Value, known: &Value, path: &str) -> Checked {
    let at = |key: &str| if path.is_empty() { key.to_owned() } else { format!("{path}.{key}") };
    match (given, known) {
        (Value::Object(given), Value::Object(known)) => {
            given.iter().try_for_each(|(key, v)| match known.get(key) {
                Some(k) => unknown(v, k, &at(key)),
                None => ensure(v.is_null(), || format!("{}: unknown field", at(key))),
            })
        }
        (Value::Array(given), Value::Array(known)) => given
            .iter()
            .zip(known)
            .enumerate()
            .try_for_each(|(i, (g, k))| unknown(g, k, &format!("{path}[{i}]"))),
        _ => Ok(()),
    }
}

impl Edit {
    pub fn parse(text: &str) -> Result<Edit, String> {
        Edit::from_value(serde_json::from_str(text).map_err(|e| e.to_string())?)
    }

    pub fn from_value(value: Value) -> Result<Edit, String> {
        let version = value.get("version").and_then(Value::as_u64).unwrap_or_default();
        ensure(version <= u64::from(VERSION), || format!("version {version} is newer than {VERSION}"))?;
        let edit: Edit = super::fields::typed(&value)?;
        unknown(&value, &serde_json::to_value(&edit).map_err(|e| e.to_string())?, "")?;
        edit.validate().map(|()| edit)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default() + "\n"
    }

    pub fn validate(&self) -> Checked {
        ensure((1..=VERSION).contains(&self.version), || format!("version: use 1 to {VERSION}"))?;
        ensure(self.source.name.len() <= 255 && self.source.sha256.len() <= 64, || {
            "source: too long".into()
        })?;
        let g = &self.geometry;
        g.crop.map_or(Ok(()), |c| rect("geometry.crop", c))?;
        ensure(g.rotate.is_multiple_of(90) && g.rotate < 360, || {
            "geometry.rotate: use 0, 90, 180 or 270".into()
        })?;
        within("geometry.straighten", g.straighten, (-45.0, 45.0))?;
        ensure(self.ops.len() <= 24 && self.masks.len() <= 8, || "use at most 24 ops and 8 masks".into())?;
        let brushes = self.masks.values().filter_map(|m| match m {
            Mask::Brush { strokes, .. } => Some(strokes.iter().map(|s| s.points.len()).sum::<usize>()),
            _ => None,
        });
        let (count, points) = brushes.fold((0, 0), |(n, p), points| (n + 1, p + points));
        ensure(count <= 4 && points <= 20_000, || "use at most 4 brush masks and 20000 points".into())?;
        let mut ids = HashSet::new();
        for (i, op) in self.ops.iter().enumerate() {
            let at = |field: &str| format!("ops[{i}].{field}");
            name(&at("id"), &op.id)?;
            ensure(ids.insert(&op.id), || format!("{}: {} is used twice", at("id"), op.id))?;
            if let Some(mask) = op.mask.as_ref().filter(|m| !self.masks.contains_key(*m)) {
                return Err(format!("{}: no mask is named {mask}", at("mask")));
            }
            op.opacity.map_or(Ok(()), |o| within(&at("opacity"), o, UNIT))?;
            match &op.kind {
                Kind::Exposure { ev } => within(&at("ev"), *ev, (-5.0, 5.0))?,
                Kind::WhiteBalance { temperature, tint } => {
                    within(&at("temperature"), *temperature, (2000.0, 12000.0))?;
                    within(&at("tint"), *tint, PERCENT)?
                }
                Kind::Contrast { amount } => within(&at("amount"), *amount, PERCENT)?,
                Kind::Saturation { saturation, vibrance } => {
                    within(&at("saturation"), *saturation, PERCENT)?;
                    within(&at("vibrance"), *vibrance, PERCENT)?
                }
                Kind::Curves { rgb, r, g, b } => [("rgb", rgb), ("r", r), ("g", g), ("b", b)]
                    .into_iter()
                    .try_for_each(|(channel, points)| curve(&at(channel), points))?,
                Kind::Blur { radius } => within(&at("radius"), *radius, (0.0, 200.0))?,
                Kind::Overlay { color, .. } => {
                    ensure(parse_hex_color(color).is_some(), || format!("{}: use #rrggbb", at("color")))?
                }
                Kind::Vignette { amount, midpoint, roundness, feather } => {
                    within(&at("amount"), *amount, (-1.0, 1.0))?;
                    all(&at("midpoint"), &[*midpoint, *roundness, *feather], UNIT)?
                }
            }
        }
        for (key, mask) in &self.masks {
            let at = format!("masks.{key}");
            name(&at, key)?;
            match mask {
                Mask::Linear { from, to, .. } => all(&at, &[*from, *to].concat(), WIDE)?,
                Mask::Radial { center, radius, feather, .. } => {
                    all(&at, center, WIDE)?;
                    all(&at, &[radius[0], radius[1], *feather], (0.0, 2.0))?
                }
                Mask::Luminance { range, soft, .. } | Mask::Depth { range, soft, .. } => {
                    all(&at, &[range[0], range[1], *soft], UNIT)?
                }
                Mask::Brush { strokes, .. } => strokes.iter().try_for_each(|s| {
                    within(&at, s.radius, (0.0, 0.5))?;
                    within(&at, s.hardness, UNIT)?;
                    all(&at, s.points.as_flattened(), WIDE)
                })?,
            }
        }
        for (key, out) in &self.outputs {
            ensure(key.len() <= 128 && out.connector.len() <= 128, || format!("outputs.{key}: too long"))?;
            rect(&format!("outputs.{key}.crop"), out.crop)?;
        }
        super::depth::check(self.depth.as_ref(), self.parallax.as_ref())?;
        ensure(self.fit != Some(crate::Fit::Span), || {
            "fit: span belongs to the screens, not to a picture".into()
        })?;
        let Some(span) = &self.span else { return Ok(()) };
        all("span.center", &span.center, UNIT)?;
        within("span.zoom", span.zoom, (1.0, 8.0))
    }
}
