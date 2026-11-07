use super::action::{Action, Error};
use super::depth::Look;
use super::parse::{invalid, numbers, usage};
use super::tool::Turn;
use super::{Edit, STRENGTH, Span};
use crate::message::said;

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
                let (key, value) = change
                    .split_once('=')
                    .ok_or_else(|| usage(said("edit.key_value", "%1 is not KEY=VALUE", &[change])))?;
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
                found
                    .then_some(())
                    .ok_or_else(|| invalid(said("edit.no_mask_named", "no mask is named %1", &[&name])))
            }
            Action::Mask { name, spec, invert } => self.shape(&name, &spec, invert),
            Action::Order { ids } => match ids.iter().find(|id| !self.ops.iter().any(|op| op.id == **id)) {
                Some(id) => Err(invalid(said("edit.no_operation", "no operation is named %1", &[id]))),
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
            Action::Fit { mode } => {
                self.fit = (mode != "none")
                    .then(|| mode.parse())
                    .transpose()
                    .map_err(|e: crate::fit::UnknownFit| usage(said("edit.fit", "%1", &[&e])))?;
                Ok(())
            }
            Action::DepthBrush { points, by, raise, lower, radius, falloff, clear } => self.touch(
                false,
                clear,
                points.map(|p| (p, by.unwrap_or(STRENGTH))),
                [raise, lower],
                (radius, falloff),
            ),
            Action::DepthCuts { cuts, auto } => self.cut(cuts, auto),
            Action::FillMask { points, refill, protect, radius, falloff, clear } => {
                self.touch(true, clear, points.map(|p| (p, 1.0)), [refill, protect], (radius, falloff))
            }
            Action::Reset { ids } => {
                let before = self.ops.len();
                self.ops.retain(|op| !ids.contains(&op.id));
                let all = before - self.ops.len() == ids.len();
                all.then_some(()).ok_or_else(|| {
                    invalid(said("edit.not_operations", "some of %1 are not operations", &[&ids.join(" ")]))
                })
            }
        }
    }
}
