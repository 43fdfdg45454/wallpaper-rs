use crate::deep::REFERENCE_NITS;
use crate::{RenderError, Renderer, Surface};
use wgpu::{
    SurfaceCapabilities, SurfaceColorSpace as Space, SurfaceColorSpaces as Spaces, TextureFormat as Format,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Encoding {
    #[default]
    Sdr,
    Scrgb,
    Pq,
    OwnPq,
}

impl Encoding {
    pub fn name(self) -> &'static str {
        match self {
            Encoding::Sdr => "sdr",
            Encoding::Scrgb => "scrgb",
            Encoding::Pq => "pq",
            Encoding::OwnPq => "pq-own",
        }
    }

    pub fn hdr(self) -> bool {
        self != Encoding::Sdr
    }

    pub fn pq(self) -> bool {
        matches!(self, Encoding::Pq | Encoding::OwnPq)
    }
}

pub type Lit = (Format, Space, Encoding);

pub fn lit(caps: &SurfaceCapabilities, wanted: bool, own: bool) -> (bool, Option<Lit>) {
    let has = |format: Format, spaces: Spaces| {
        caps.format_capabilities.iter().any(|f| f.format == format && f.color_spaces.contains(spaces))
    };
    let ways = [
        (Format::Rgba16Float, Spaces::EXTENDED_SRGB_LINEAR, Space::ExtendedSrgbLinear, Encoding::Scrgb),
        (Format::Rgb10a2Unorm, Spaces::BT2100_PQ, Space::Bt2100Pq, Encoding::Pq),
        (Format::Rgba16Float, Spaces::BT2100_PQ, Space::Bt2100Pq, Encoding::Pq),
        (Format::Rgb10a2Unorm, Spaces::SRGB, Space::Srgb, Encoding::OwnPq),
    ];
    let usable = ways.into_iter().filter(|w| own || w.3 != Encoding::OwnPq);
    let found = usable.into_iter().find(|(f, s, ..)| has(*f, *s)).map(|(f, _, s, e)| (f, s, e));
    (found.is_some(), found.filter(|_| wanted))
}

impl Surface {
    pub fn hdr(&self) -> bool {
        self.encoding.hdr()
    }

    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    pub fn hdr_offered(&self) -> bool {
        self.offered
    }

    pub fn set_light(&mut self, reference_nits: f32, headroom: f32) {
        let fine = |v: f32, lo: f32, hi: f32, or: f32| if v.is_finite() { v.clamp(lo, hi) } else { or };
        self.light = [
            fine(reference_nits, 1.0, 10_000.0, REFERENCE_NITS),
            fine(headroom, 1.0, crate::hdr::MAX_HEADROOM, 1.0),
        ];
    }
}

impl Renderer {
    pub fn own_pq(&self, target: &mut Surface, own: bool) -> Result<(), RenderError> {
        if target.presentation.own_pq == own {
            return Ok(());
        }
        target.presentation.own_pq = own;
        self.renew(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(list: &[(Format, Spaces)]) -> SurfaceCapabilities {
        let caps = list.iter().map(|(format, color_spaces)| wgpu::SurfaceFormatCapabilities {
            format: *format,
            color_spaces: *color_spaces,
        });
        SurfaceCapabilities { format_capabilities: caps.collect(), ..Default::default() }
    }

    #[test]
    fn hdr_takes_scrgb_then_pq_then_a_description_of_its_own() {
        let scrgb = with(&[(Format::Rgba16Float, Spaces::SRGB | Spaces::EXTENDED_SRGB_LINEAR)]);
        let linear = (Format::Rgba16Float, Space::ExtendedSrgbLinear, Encoding::Scrgb);
        assert_eq!(lit(&scrgb, true, true), (true, Some(linear)));
        assert_eq!(lit(&scrgb, false, false), (true, None));
        let pq = with(&[(Format::Rgb10a2Unorm, Spaces::SRGB | Spaces::BT2100_PQ)]);
        assert_eq!(
            lit(&pq, true, false),
            (true, Some((Format::Rgb10a2Unorm, Space::Bt2100Pq, Encoding::Pq)))
        );
        let plain = with(&[(Format::Rgb10a2Unorm, Spaces::SRGB), (Format::Bgra8UnormSrgb, Spaces::SRGB)]);
        assert_eq!(lit(&plain, true, false), (false, None));
        let own = (Format::Rgb10a2Unorm, Space::Srgb, Encoding::OwnPq);
        assert_eq!(lit(&plain, true, true), (true, Some(own)));
        assert_eq!(lit(&with(&[(Format::Rgba16Float, Spaces::SRGB)]), true, true), (false, None));
        assert_eq!(lit(&SurfaceCapabilities::default(), true, true), (false, None));
        assert!(
            Encoding::OwnPq.pq() && Encoding::Scrgb.hdr() && !Encoding::Sdr.hdr() && !Encoding::Scrgb.pq()
        );
        assert_eq!(Encoding::OwnPq.name(), "pq-own");
    }
}
