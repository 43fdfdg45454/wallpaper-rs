use super::check::bounded;
use super::parse::{invalid, numbers, usage};
use super::{Edit, Error};
use crate::message::{Message, said};
use serde::{Deserialize, Serialize};

pub const MOST_POINTS: usize = 3000;
pub const MOST_STROKE: usize = 1024;
pub const MOST_STROKES: usize = 256;
pub const MOST_CUTS: usize = 5;
pub const GAP: f32 = 0.005;
pub const RADIUS: f32 = 0.05;
pub const FALLOFF: f32 = 0.5;
pub const STRENGTH: f32 = 0.1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Touch {
    pub points: Vec<[f32; 2]>,
    pub radius: f32,
    pub falloff: f32,
    pub by: f32,
}

pub fn points(list: &str) -> Result<Vec<[f32; 2]>, Error> {
    let found = list.split(';').filter(|p| !p.trim().is_empty()).map(numbers::<2>);
    let found = found.collect::<Result<Vec<_>, _>>()?;
    let fine = (1..=MOST_STROKE).contains(&found.len());
    let said = || said("edit.stroke_points", "a stroke has 1 to %1 points", &[&MOST_STROKE]);
    fine.then_some(found).ok_or_else(|| usage(said()))
}

pub(super) fn check(path: &str, touches: &[Touch]) -> Result<(), Message> {
    let total: usize = touches.iter().map(|t| t.points.len()).sum();
    let fine = touches.len() <= MOST_STROKES && total <= MOST_POINTS;
    let crowded = || {
        said(
            "edit.strokes",
            "use at most %1 strokes and %2 points in %3",
            &[&MOST_STROKES, &MOST_POINTS, &path],
        )
    };
    fine.then_some(()).ok_or_else(crowded)?;
    for t in touches {
        let few = (1..=MOST_STROKE).contains(&t.points.len());
        few.then_some(())
            .ok_or_else(|| said("edit.stroke_points", "a stroke has 1 to %1 points", &[&MOST_STROKE]))?;
        t.points.as_flattened().iter().try_for_each(|v| bounded(path, *v, (0.0, 1.0)))?;
        bounded(path, t.radius, (0.001, 1.0))?;
        bounded(path, t.falloff, (0.0, 1.0))?;
        bounded(path, t.by, (-1.0, 1.0))?;
    }
    Ok(())
}

pub(super) fn cuts(found: &[f32]) -> Result<(), Message> {
    let fine = (1..=MOST_CUTS).contains(&found.len());
    fine.then_some(())
        .ok_or_else(|| said("edit.cuts_count", "depth.cuts: use 1 to %1 cuts", &[&MOST_CUTS]))?;
    found.iter().try_for_each(|c| bounded("depth.cuts", *c, (GAP, 1.0 - GAP)))?;
    let apart = found.windows(2).all(|w| w[1] - w[0] >= GAP);
    apart
        .then_some(())
        .ok_or_else(|| said("edit.cuts_order", "depth.cuts: use growing cuts at least %1 apart", &[&GAP]))
}

impl Edit {
    pub(super) fn touch(
        &mut self,
        fills: bool,
        clear: bool,
        new: Option<(String, f32)>,
        [up, down]: [bool; 2],
        (radius, falloff): (Option<f32>, Option<f32>),
    ) -> Result<(), Error> {
        if new.is_none() && !clear {
            return Err(usage(said("edit.touch_none", "give a stroke, or clear to remove the strokes", &[])));
        }
        let touch = new.map(|(list, by)| {
            let signed = match (up, down) {
                (false, false) if !fills => Some(by.abs()),
                (true, false) | (false, true) => Some(if down { -by.abs() } else { by.abs() }),
                _ => None,
            };
            let said =
                said("edit.touch_sign", "choose one of --raise and --lower, or --refill and --protect", &[]);
            let by = signed.ok_or_else(|| usage(said))?;
            let (radius, falloff) = (radius.unwrap_or(RADIUS), falloff.unwrap_or(FALLOFF));
            Ok::<_, Error>(Touch { points: points(&list)?, radius, falloff, by })
        });
        let touch = touch.transpose()?;
        let d = self.depth.get_or_insert_with(super::Depth::default);
        let list = if fills { &mut d.fills } else { &mut d.strokes };
        if clear {
            list.clear();
        }
        list.extend(touch);
        Ok(())
    }

    pub(super) fn cut(&mut self, list: Option<String>, auto: bool) -> Result<(), Error> {
        let parsed =
            list.map(|l| l.split(',').map(|n| n.trim().parse::<f32>()).collect::<Result<Vec<_>, _>>());
        let parsed = parsed
            .transpose()
            .map_err(|_| usage(said("edit.cuts_list", "use cuts such as 0.2,0.45,0.7", &[])))?;
        if parsed.is_none() && !auto {
            return Err(usage(said("edit.cuts_none", "give the cuts or auto", &[])));
        }
        let found = parsed.filter(|_| !auto);
        found.as_deref().map_or(Ok(()), |c| cuts(c).map_err(invalid))?;
        self.depth.get_or_insert_with(super::Depth::default).cuts = found;
        Ok(())
    }
}
