use super::depth::Look;
use super::parse::{invalid, numbers, usage};
use super::tool::Turn;
use super::{Edit, Mask, Span};
use serde::Deserialize;

pub const ACTIONS: [&str; 12] = [
    "set", "crop", "curve", "blur", "vignette", "overlay", "mask", "order", "span", "reset", "depth",
    "parallax",
];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Set {
        changes: Vec<String>,
    },
    Crop {
        rect: Option<String>,
        aspect: Option<String>,
        rotate: Option<u16>,
        straighten: Option<f32>,
        flip: Option<String>,
        output: Option<String>,
    },
    Curve {
        #[serde(default)]
        points: Vec<String>,
        channel: Option<String>,
        id: Option<String>,
    },
    Blur {
        radius: Option<f32>,
        mask: Option<String>,
        id: Option<String>,
    },
    Vignette {
        amount: Option<f32>,
        midpoint: Option<f32>,
        roundness: Option<f32>,
        feather: Option<f32>,
        id: Option<String>,
    },
    Overlay {
        color: Option<String>,
        blend: Option<String>,
        opacity: Option<f32>,
        mask: Option<String>,
        id: Option<String>,
    },
    Mask {
        name: String,
        spec: String,
        #[serde(default)]
        invert: bool,
    },
    Order {
        ids: Vec<String>,
    },
    Span {
        zoom: Option<f32>,
        center: Option<String>,
    },
    Reset {
        #[serde(default)]
        ids: Vec<String>,
    },
    Depth {
        near: Option<f32>,
        far: Option<f32>,
        gamma: Option<f32>,
        smooth: Option<u32>,
        dilate: Option<u32>,
        input: Option<u32>,
    },
    Parallax {
        source: Option<String>,
        strength: Option<f32>,
        focus: Option<f32>,
        steps: Option<u32>,
        smoothing: Option<u32>,
        overscan: Option<f32>,
        #[serde(default)]
        off: bool,
    },
}

impl Edit {
    pub fn act(&mut self, action: Action) -> Result<(), Error> {
        let mut next = self.clone();
        next.run(action)?;
        next.validate().map_err(invalid)?;
        *self = next;
        Ok(())
    }

    fn run(&mut self, action: Action) -> Result<(), Error> {
        match action {
            Action::Set { changes } => changes.iter().try_for_each(|change| {
                let (key, value) =
                    change.split_once('=').ok_or_else(|| usage(format!("{change} is not KEY=VALUE")))?;
                self.set(key, value).map_err(invalid)
            }),
            Action::Crop { rect, aspect, rotate, straighten, flip, output } => {
                self.crop(rect, aspect, Turn { rotate, straighten, flip }, output)
            }
            Action::Curve { points, channel, id } => self.curve(&points, channel, id),
            Action::Blur { radius, mask, id } => self.blur(radius, mask, id),
            Action::Vignette { amount, midpoint, roundness, feather, id } => {
                self.vignette([amount, midpoint, roundness, feather], id)
            }
            Action::Overlay { color, blend, opacity, mask, id } => {
                self.overlay(color, blend, opacity, mask, id)
            }
            Action::Mask { name, spec, .. } if spec == "none" => {
                self.ops
                    .iter_mut()
                    .filter(|op| op.mask.as_ref() == Some(&name))
                    .for_each(|op| op.mask = None);
                let found = self.masks.remove(&name).is_some();
                found.then_some(()).ok_or_else(|| invalid(format!("no mask is named {name}")))
            }
            Action::Mask { name, spec, invert } => {
                self.masks.insert(name, Mask::parse(&spec, invert)?);
                Ok(())
            }
            Action::Order { ids } => match ids.iter().find(|id| !self.ops.iter().any(|op| op.id == **id)) {
                Some(id) => Err(invalid(format!("no operation is named {id}"))),
                None => {
                    self.ops.sort_by_key(|op| ids.iter().position(|id| *id == op.id).unwrap_or(usize::MAX));
                    Ok(())
                }
            },
            Action::Span { zoom, center } => {
                let center = center.as_deref().map(numbers::<2>).transpose()?;
                let span = self.span.get_or_insert_with(Span::default);
                (span.zoom, span.center) = (zoom.unwrap_or(span.zoom), center.unwrap_or(span.center));
                Ok(())
            }
            Action::Reset { ids } if ids.is_empty() => {
                *self = Edit { source: self.source.clone(), ..Edit::default() };
                Ok(())
            }
            Action::Depth { near, far, gamma, smooth, dilate, input } => {
                self.depth([near, far, gamma], [smooth, dilate, input]);
                Ok(())
            }
            Action::Parallax { source, strength, focus, steps, smoothing, overscan, off } => {
                self.parallax(Look { source, strength, focus, steps, smoothing, overscan, off })
            }
            Action::Reset { ids } => {
                let before = self.ops.len();
                self.ops.retain(|op| !ids.contains(&op.id));
                let all = before - self.ops.len() == ids.len();
                all.then_some(())
                    .ok_or_else(|| invalid(format!("some of {} are not operations", ids.join(" "))))
            }
        }
    }
}
