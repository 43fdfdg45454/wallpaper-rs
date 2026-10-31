use super::{Edit, Kind, Mask, VERSION};
use crate::config::parse_hex_color;
use crate::message::{Message, said};
use serde_json::Value;
use std::collections::HashSet;

const PERCENT: (f32, f32) = (-100.0, 100.0);
const UNIT: (f32, f32) = (0.0, 1.0);
const WIDE: (f32, f32) = (-1.0, 2.0);

type Checked = Result<(), Message>;

pub(super) fn bounded(path: &str, value: f32, (lo, hi): (f32, f32)) -> Checked {
    let fine = (lo..=hi).contains(&value);
    let out = || said("edit.range", "%1: %2 is out of range %3 to %4", &[&path, &value, &lo, &hi]);
    fine.then_some(()).ok_or_else(out)
}

fn all(path: &str, values: &[f32], range: (f32, f32)) -> Checked {
    values.iter().try_for_each(|v| bounded(path, *v, range))
}

fn ensure(fine: bool, message: impl FnOnce() -> Message) -> Checked {
    fine.then_some(()).ok_or_else(message)
}

fn name(path: &str, text: &str) -> Checked {
    let fine = text.chars().all(|c| c.is_ascii_alphanumeric() || "_-".contains(c));
    ensure((1..=64).contains(&text.len()) && fine, || {
        said("edit.name", "%1: use 1 to 64 letters, digits, - or _", &[&path])
    })
}

fn rect(path: &str, [x, y, w, h]: [f32; 4]) -> Checked {
    all(path, &[x, y, w, h], UNIT)?;
    ensure(w > 0.0 && h > 0.0 && x + w <= 1.0001 && y + h <= 1.0001, || {
        said("edit.outside", "%1: not inside the image", &[&path])
    })
}

fn curve(path: &str, points: &[[f32; 2]]) -> Checked {
    ensure(points.is_empty() || (2..=16).contains(&points.len()), || {
        said("edit.points", "%1: use 2 to 16 points", &[&path])
    })?;
    all(path, points.as_flattened(), UNIT)?;
    ensure(points.windows(2).all(|p| p[0][0] < p[1][0]), || {
        said("edit.growing", "%1: x must grow from point to point", &[&path])
    })
}

fn unknown(given: &Value, known: &Value, path: &str) -> Checked {
    let at = |key: &str| if path.is_empty() { key.to_owned() } else { format!("{path}.{key}") };
    match (given, known) {
        (Value::Object(given), Value::Object(known)) => {
            given.iter().try_for_each(|(key, v)| match known.get(key) {
                Some(k) => unknown(v, k, &at(key)),
                None => ensure(v.is_null(), || said("edit.unknown_field", "%1: unknown field", &[&at(key)])),
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
    pub fn parse(text: &str) -> Result<Edit, Message> {
        Edit::from_value(serde_json::from_str(text).map_err(|e| said("edit.json", "%1", &[&e]))?)
    }

    pub fn from_value(value: Value) -> Result<Edit, Message> {
        let version = value.get("version").and_then(Value::as_u64).unwrap_or_default();
        ensure(version <= u64::from(VERSION), || {
            said("edit.newer", "version %1 is newer than %2", &[&version, &VERSION])
        })?;
        let edit: Edit = super::fields::typed(&value)?;
        let known = serde_json::to_value(&edit).map_err(|e| said("edit.json", "%1", &[&e]))?;
        unknown(&value, &known, "")?;
        edit.validate().map(|()| edit)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default() + "\n"
    }

    pub fn validate(&self) -> Checked {
        ensure((1..=VERSION).contains(&self.version), || {
            said("edit.version", "version: use 1 to %1", &[&VERSION])
        })?;
        ensure(self.source.name.len() <= 255 && self.source.sha256.len() <= 64, || {
            said("edit.source_long", "source: too long", &[])
        })?;
        let g = &self.geometry;
        g.crop.map_or(Ok(()), |c| rect("geometry.crop", c))?;
        ensure(g.rotate.is_multiple_of(90) && g.rotate < 360, || {
            said("edit.rotate", "geometry.rotate: use 0, 90, 180 or 270", &[])
        })?;
        bounded("geometry.straighten", g.straighten, (-45.0, 45.0))?;
        ensure(self.ops.len() <= 24 && self.masks.len() <= 8, || {
            said("edit.crowded", "use at most 24 ops and 8 masks", &[])
        })?;
        let brushes = self.masks.values().filter_map(|m| match m {
            Mask::Brush { strokes, .. } => Some(strokes.iter().map(|s| s.points.len()).sum::<usize>()),
            _ => None,
        });
        let (count, points) = brushes.fold((0, 0), |(n, p), points| (n + 1, p + points));
        ensure(count <= 4 && points <= 20_000, || {
            said("edit.brushes", "use at most 4 brush masks and 20000 points", &[])
        })?;
        let mut ids = HashSet::new();
        for (i, op) in self.ops.iter().enumerate() {
            let at = |field: &str| format!("ops[{i}].{field}");
            name(&at("id"), &op.id)?;
            ensure(ids.insert(&op.id), || said("edit.twice", "%1: %2 is used twice", &[&at("id"), &op.id]))?;
            if let Some(mask) = op.mask.as_ref().filter(|m| !self.masks.contains_key(*m)) {
                return Err(said("edit.no_mask", "%1: no mask is named %2", &[&at("mask"), mask]));
            }
            op.opacity.map_or(Ok(()), |o| bounded(&at("opacity"), o, UNIT))?;
            match &op.kind {
                Kind::Exposure { ev } => bounded(&at("ev"), *ev, (-5.0, 5.0))?,
                Kind::WhiteBalance { temperature, tint } => {
                    bounded(&at("temperature"), *temperature, (2000.0, 12000.0))?;
                    bounded(&at("tint"), *tint, PERCENT)?
                }
                Kind::Contrast { amount } => bounded(&at("amount"), *amount, PERCENT)?,
                Kind::Saturation { saturation, vibrance } => {
                    bounded(&at("saturation"), *saturation, PERCENT)?;
                    bounded(&at("vibrance"), *vibrance, PERCENT)?
                }
                Kind::Curves { rgb, r, g, b } => [("rgb", rgb), ("r", r), ("g", g), ("b", b)]
                    .into_iter()
                    .try_for_each(|(channel, points)| curve(&at(channel), points))?,
                Kind::Blur { radius } => bounded(&at("radius"), *radius, (0.0, 200.0))?,
                Kind::Overlay { color, .. } => ensure(parse_hex_color(color).is_some(), || {
                    said("edit.color", "%1: use #rrggbb", &[&at("color")])
                })?,
                Kind::Vignette { amount, midpoint, roundness, feather } => {
                    bounded(&at("amount"), *amount, (-1.0, 1.0))?;
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
                    bounded(&at, s.radius, (0.0, 0.5))?;
                    bounded(&at, s.hardness, UNIT)?;
                    all(&at, s.points.as_flattened(), WIDE)
                })?,
            }
        }
        for (key, out) in &self.outputs {
            ensure(key.len() <= 128 && out.connector.len() <= 128, || {
                said("edit.output_long", "outputs.%1: too long", &[key])
            })?;
            rect(&format!("outputs.{key}.crop"), out.crop)?;
        }
        super::depth::check(self.depth.as_ref(), self.parallax.as_ref())?;
        ensure(self.fit != Some(crate::Fit::Span), || {
            said("edit.span_fit", "fit: span belongs to the screens, not to a picture", &[])
        })?;
        let Some(span) = &self.span else { return Ok(()) };
        all("span.center", &span.center, UNIT)?;
        bounded("span.zoom", span.zoom, (1.0, 8.0))
    }
}
