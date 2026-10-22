use super::parse::{blend, fit, flips, invalid, numbers, ratio, usage};
use super::{Edit, Error, Geometry, Kind, Mask, Op, OutputCrop};

const WHOLE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

pub struct Turn {
    pub rotate: Option<u16>,
    pub straighten: Option<f32>,
    pub flip: Option<String>,
}

impl Edit {
    fn kind(&self, id: &str) -> Option<Kind> {
        self.ops.iter().find(|op| op.id == id).map(|op| op.kind.clone())
    }

    fn upsert(&mut self, id: &str, kind: Kind, mask: Option<String>, opacity: Option<f32>) {
        match self.ops.iter_mut().find(|op| op.id == id) {
            Some(op) => {
                (op.kind, op.mask, op.opacity) = (kind, mask.or(op.mask.take()), opacity.or(op.opacity))
            }
            None => self.ops.push(Op { id: id.into(), enabled: true, mask, opacity, kind }),
        }
    }

    fn mask_for(&mut self, id: &str, spec: Option<String>) -> Result<Option<String>, Error> {
        match spec {
            Some(spec) if spec.contains(':') => {
                self.masks.insert(id.into(), Mask::parse(&spec, false)?);
                Ok(Some(id.into()))
            }
            name => Ok(name),
        }
    }

    pub(super) fn crop(
        &mut self,
        rect: Option<String>,
        aspect: Option<String>,
        turn: Turn,
        output: Option<String>,
    ) -> Result<(), Error> {
        let rect = match rect.as_deref() {
            None => None,
            Some("none") => Some(None),
            Some(rect) => Some(Some(numbers::<4>(rect)?)),
        };
        let (aspect, pixels) = (aspect.as_deref().map(ratio).transpose()?, self.source.pixels.into());
        let g = &mut self.geometry;
        (g.rotate, g.straighten) = (turn.rotate.unwrap_or(g.rotate), turn.straighten.unwrap_or(g.straighten));
        g.flip = turn.flip.as_deref().map(flips).transpose()?.unwrap_or(g.flip);
        let Some(output) = output else {
            let frame = Geometry { crop: None, ..g.clone() }.frame(pixels);
            let area = rect.unwrap_or(g.crop);
            g.crop = aspect.map(|r| fit(area.unwrap_or(WHOLE), r, frame)).or(area);
            return Ok(());
        };
        let frame = g.frame(pixels);
        match rect.unwrap_or_else(|| Some(self.outputs.get(&output).map_or(WHOLE, |o| o.crop))) {
            None => drop(self.outputs.remove(&output)),
            Some(area) => {
                let crop = aspect.map_or(area, |r| fit(area, r, frame));
                self.outputs.insert(output.clone(), OutputCrop { connector: output, crop });
            }
        }
        Ok(())
    }

    pub(super) fn curve(
        &mut self,
        points: &[String],
        channel: Option<String>,
        id: Option<String>,
    ) -> Result<(), Error> {
        let mut points = points.iter().map(|p| numbers::<2>(p)).collect::<Result<Vec<_>, _>>()?;
        points.sort_by(|a, b| a[0].total_cmp(&b[0]));
        let (id, channel) = (id.unwrap_or_else(|| "cu".into()), channel.unwrap_or_else(|| "rgb".into()));
        if !["rgb", "r", "g", "b"].contains(&channel.as_str()) {
            return Err(usage(format!("{channel} is not rgb, r, g or b")));
        }
        if !matches!(self.kind(&id), Some(Kind::Curves { .. })) {
            let (rgb, r, g, b) = Default::default();
            self.upsert(&id, Kind::Curves { rgb, r, g, b }, None, None);
        }
        let points = serde_json::to_string(&points).map_err(|e| invalid(e.to_string()))?;
        self.set(&format!("ops.{id}.{channel}"), &points).map_err(invalid)
    }

    pub(super) fn blur(
        &mut self,
        radius: Option<f32>,
        mask: Option<String>,
        id: Option<String>,
    ) -> Result<(), Error> {
        let id = id.unwrap_or_else(|| "bl".into());
        let old = match self.kind(&id) {
            Some(Kind::Blur { radius }) => radius,
            _ => 0.0,
        };
        let mask = self.mask_for(&id, mask)?;
        self.upsert(&id, Kind::Blur { radius: radius.unwrap_or(old) }, mask, None);
        Ok(())
    }

    pub(super) fn vignette(&mut self, given: [Option<f32>; 4], id: Option<String>) -> Result<(), Error> {
        let id = id.unwrap_or_else(|| "vi".into());
        let old = match self.kind(&id) {
            Some(Kind::Vignette { amount, midpoint, roundness, feather }) => {
                [amount, midpoint, roundness, feather]
            }
            _ => [0.0, 0.5, 0.0, 0.5],
        };
        let [amount, midpoint, roundness, feather] = std::array::from_fn(|i| given[i].unwrap_or(old[i]));
        self.upsert(&id, Kind::Vignette { amount, midpoint, roundness, feather }, None, None);
        Ok(())
    }

    pub(super) fn overlay(
        &mut self,
        color: Option<String>,
        mode: Option<String>,
        opacity: Option<f32>,
        mask: Option<String>,
        id: Option<String>,
    ) -> Result<(), Error> {
        let id = id.unwrap_or_else(|| "ov".into());
        let (old, was) = match self.kind(&id) {
            Some(Kind::Overlay { color, blend }) => (color, blend),
            _ => ("#000000".into(), Default::default()),
        };
        let blend = mode.as_deref().map(blend).transpose()?.unwrap_or(was);
        let mask = self.mask_for(&id, mask)?;
        self.upsert(&id, Kind::Overlay { color: color.unwrap_or(old), blend }, mask, opacity);
        Ok(())
    }
}
