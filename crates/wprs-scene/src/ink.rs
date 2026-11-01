use crate::resolve::Context;
use crate::typeset::{Glyphs, TextLayer};
use crate::{Color, Quality};

const LOW_BLUR: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ink {
    pub outline: Option<(Color, f32)>,
    pub shadow: Option<(Color, [f32; 2], f32)>,
    pub blur: f32,
    pub field: bool,
    pub reach: f32,
}

impl TextLayer {
    pub(crate) fn spacing(&self, cx: &Context) -> f32 {
        self.letter_spacing.as_ref().map_or(0.0, |v| v.get(cx).clamp(-100.0, 1000.0))
    }

    pub(crate) fn ink(&self, cx: &Context, quality: Quality) -> Option<Ink> {
        let field = self.render == Some(Glyphs::Sdf);
        let drawn = self.outline.is_some() || self.shadow.is_some() || self.blur.is_some();
        if !(drawn || field) {
            return None;
        }
        let outline = self.outline.as_ref().map(|o| (o.color.get(cx), o.width.get(cx).clamp(0.0, 64.0)));
        let low = |b: f32| if quality == Quality::Q0 { b.min(LOW_BLUR) } else { b };
        let shadow = self.shadow.as_ref().map(|s| {
            let offset = s.offset.get(cx).0.map(|v| v.clamp(-1024.0, 1024.0));
            (s.color.get(cx), offset, low(s.blur.get(cx).clamp(0.0, 64.0)))
        });
        let blur = self.blur.as_ref().map_or(0.0, |v| v.get(cx).clamp(0.0, 64.0));
        let width = outline.map_or(0.0, |o| o.1);
        let cast = shadow.map_or(0.0, |(_, [x, y], b)| x.abs().max(y.abs()) + b + width);
        let reach = 2f32.powf((width.max(cast).max(blur) + 2.0).log2().ceil()).max(4.0);
        let field = field || (drawn && self.render != Some(Glyphs::Raster));
        Some(Ink { outline, shadow, blur, field, reach })
    }
}
