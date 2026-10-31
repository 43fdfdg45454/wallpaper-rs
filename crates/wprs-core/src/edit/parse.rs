use super::{Blend, Error, Mask, Stroke};
use crate::message::{Message, said};

pub fn usage(message: Message) -> Error {
    Error::Usage(message)
}

pub fn invalid(message: Message) -> Error {
    Error::Invalid(message)
}

pub fn numbers<const N: usize>(list: &str) -> Result<[f32; N], Error> {
    let parsed: Result<Vec<f32>, _> = list.split(',').map(|n| n.trim().parse::<f32>()).collect();
    let parsed = parsed.map_err(|_| usage(said("edit.numbers", "%1 is not a list of numbers", &[&list])))?;
    parsed.try_into().map_err(|_| usage(said("edit.count", "%1 does not have %2 numbers", &[&list, &N])))
}

pub fn ratio(text: &str) -> Result<f32, Error> {
    let (w, h) = text.split_once(':').unwrap_or((text, "1"));
    let ratio = w.trim().parse::<f32>().ok().zip(h.trim().parse::<f32>().ok()).map(|(w, h)| w / h);
    let fine = ratio.filter(|r| (0.05..=20.0).contains(r));
    fine.ok_or_else(|| usage(said("edit.ratio", "%1 is not a ratio such as 16:9", &[&text])))
}

pub fn flips(text: &str) -> Result<[bool; 2], Error> {
    match text {
        "none" | "h" | "v" | "hv" | "vh" => Ok([text.contains('h'), text.contains('v')]),
        _ => Err(usage(said("edit.flip", "%1 is not none, h, v or hv", &[&text]))),
    }
}

pub fn blend(text: &str) -> Result<Blend, Error> {
    let name = serde_json::Value::String(text.replace('-', "_"));
    serde_json::from_value(name).map_err(|_| usage(said("edit.blend", "%1 is not a blend mode", &[&text])))
}

pub fn fit([x, y, w, h]: [f32; 4], ratio: f32, (fw, fh): (u32, u32)) -> [f32; 4] {
    let shape = ratio * fh.max(1) as f32 / fw.max(1) as f32;
    let tall = (w * h / shape).sqrt();
    let scale = (1.0 / (shape * tall)).min(1.0 / tall).min(1.0);
    let (nw, nh) = ((shape * tall * scale).min(1.0), (tall * scale).min(1.0));
    let place = |at: f32, old: f32, new: f32| (at + (old - new) / 2.0).clamp(0.0, 1.0 - new);
    [place(x, w, nw), place(y, h, nh), nw, nh]
}

impl Stroke {
    pub fn parse(spec: &str) -> Result<Option<Stroke>, Error> {
        let Some((kind, rest)) = spec.split_once(':').filter(|(k, _)| ["brush", "erase"].contains(k)) else {
            return Ok(None);
        };
        let (shape, list) = rest
            .split_once(':')
            .ok_or_else(|| usage(said("edit.stroke", "%1 is not a brush stroke", &[&spec])))?;
        let [radius, hardness] = numbers(shape)?;
        let points = list.split(';').filter(|p| !p.trim().is_empty()).map(numbers::<2>);
        let points = points.collect::<Result<Vec<_>, _>>()?;
        Ok(Some(Stroke { radius, hardness, erase: kind == "erase", points }))
    }
}

impl Mask {
    pub fn parse(spec: &str, invert: bool) -> Result<Mask, Error> {
        let (kind, values) =
            spec.split_once(':').ok_or_else(|| usage(said("edit.mask", "%1 is not a mask", &[&spec])))?;
        let count = values.split(',').count();
        let soft = if kind == "radial" { 0.5 } else { 0.1 };
        let full = if count < 3 || kind == "radial" && count == 4 {
            format!("{values},{soft}")
        } else {
            values.into()
        };
        match kind {
            "linear" => {
                numbers(values).map(|[x0, y0, x1, y1]| Mask::Linear { from: [x0, y0], to: [x1, y1], invert })
            }
            "radial" => numbers(&full).map(|[x, y, rx, ry, feather]| Mask::Radial {
                center: [x, y],
                radius: [rx, ry],
                feather,
                invert,
            }),
            "luminance" => {
                numbers(&full).map(|[lo, hi, soft]| Mask::Luminance { range: [lo, hi], soft, invert })
            }
            "depth" => numbers(&full).map(|[lo, hi, soft]| Mask::Depth { range: [lo, hi], soft, invert }),
            _ => Err(usage(said(
                "edit.mask_kind",
                "%1 is not linear, radial, luminance, depth, brush or erase",
                &[&kind],
            ))),
        }
    }

    pub fn spec(&self) -> Option<String> {
        let list = |values: &[f32]| values.iter().map(f32::to_string).collect::<Vec<_>>().join(",");
        Some(match self {
            Mask::Linear { from, to, .. } => format!("linear:{}", list(&[*from, *to].concat())),
            Mask::Radial { center, radius, feather, .. } => {
                format!("radial:{}", list(&[center[0], center[1], radius[0], radius[1], *feather]))
            }
            Mask::Luminance { range, soft, .. } => {
                format!("luminance:{}", list(&[range[0], range[1], *soft]))
            }
            Mask::Depth { range, soft, .. } => format!("depth:{}", list(&[range[0], range[1], *soft])),
            Mask::Brush { .. } => return None,
        })
    }
}
