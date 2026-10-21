//! How an image is placed on an output.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Placement of an image on a target surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    /// Scale to cover the whole target, cropping the overflow.
    #[default]
    Fill,
    /// Scale to fit inside the target, leaving bars.
    Fit,
    /// Stretch to the target size, ignoring aspect ratio.
    Stretch,
    /// Keep the original pixel size, centered.
    Center,
    /// Repeat the image at its original pixel size.
    Tile,
}

impl Fit {
    pub const ALL: [Fit; 5] = [Fit::Fill, Fit::Fit, Fit::Stretch, Fit::Center, Fit::Tile];

    pub fn as_str(self) -> &'static str {
        match self {
            Fit::Fill => "fill",
            Fit::Fit => "fit",
            Fit::Stretch => "stretch",
            Fit::Center => "center",
            Fit::Tile => "tile",
        }
    }
}

impl fmt::Display for Fit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownFit(pub String);

impl fmt::Display for UnknownFit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown fit mode '{}' (expected fill, fit, stretch, center or tile)",
            self.0
        )
    }
}

impl std::error::Error for UnknownFit {}

impl FromStr for Fit {
    type Err = UnknownFit;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Fit::ALL
            .into_iter()
            .find(|f| f.as_str().eq_ignore_ascii_case(s.trim()))
            .ok_or_else(|| UnknownFit(s.to_owned()))
    }
}

/// Mapping from target coordinates to image texture coordinates.
///
/// A target pixel at normalized position `p` in `[0, 1]²` samples the image at
/// `offset + p * scale`. Coordinates outside `[0, 1]` are either background
/// (for `Fit` and `Center`) or wrap around (for `Tile`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvRect {
    pub offset: [f32; 2],
    pub scale: [f32; 2],
    pub repeat: bool,
}

impl UvRect {
    /// Computes the mapping for an image of `image` pixels on a target of
    /// `target` pixels. Zero sizes are treated as one pixel.
    pub fn compute(fit: Fit, image: (u32, u32), target: (u32, u32)) -> UvRect {
        let iw = image.0.max(1) as f32;
        let ih = image.1.max(1) as f32;
        let tw = target.0.max(1) as f32;
        let th = target.1.max(1) as f32;

        // Size of the image on the target, in target pixels.
        let (dw, dh, repeat) = match fit {
            Fit::Stretch => (tw, th, false),
            Fit::Fill => {
                let s = (tw / iw).max(th / ih);
                (iw * s, ih * s, false)
            }
            Fit::Fit => {
                let s = (tw / iw).min(th / ih);
                (iw * s, ih * s, false)
            }
            Fit::Center => (iw, ih, false),
            Fit::Tile => (iw, ih, true),
        };

        let scale = [tw / dw, th / dh];
        let offset = if repeat {
            [0.0, 0.0]
        } else {
            // Center the image: target center maps to image center.
            [0.5 - 0.5 * scale[0], 0.5 - 0.5 * scale[1]]
        };
        UvRect {
            offset,
            scale,
            repeat,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5
    }

    #[test]
    fn parse_roundtrip() {
        for f in Fit::ALL {
            assert_eq!(f.as_str().parse::<Fit>().unwrap(), f);
        }
        assert_eq!(" FILL ".parse::<Fit>().unwrap(), Fit::Fill);
        assert!("zoom".parse::<Fit>().is_err());
    }

    #[test]
    fn stretch_is_identity() {
        let r = UvRect::compute(Fit::Stretch, (100, 50), (1920, 1080));
        assert!(approx(r.offset, [0.0, 0.0]));
        assert!(approx(r.scale, [1.0, 1.0]));
    }

    #[test]
    fn same_aspect_is_identity_for_fill_and_fit() {
        for fit in [Fit::Fill, Fit::Fit] {
            let r = UvRect::compute(fit, (1280, 720), (1920, 1080));
            assert!(approx(r.offset, [0.0, 0.0]), "{fit}");
            assert!(approx(r.scale, [1.0, 1.0]), "{fit}");
        }
    }

    #[test]
    fn fill_crops_the_long_axis() {
        // Square image on a 2:1 target: full width, half of the height visible.
        let r = UvRect::compute(Fit::Fill, (100, 100), (200, 100));
        assert!(approx(r.scale, [1.0, 0.5]));
        assert!(approx(r.offset, [0.0, 0.25]));
    }

    #[test]
    fn fit_letterboxes_the_short_axis() {
        // Square image on a 2:1 target: bars on the left and right.
        let r = UvRect::compute(Fit::Fit, (100, 100), (200, 100));
        assert!(approx(r.scale, [2.0, 1.0]));
        assert!(approx(r.offset, [-0.5, 0.0]));
    }

    #[test]
    fn center_keeps_pixel_size() {
        let r = UvRect::compute(Fit::Center, (100, 50), (200, 100));
        assert!(approx(r.scale, [2.0, 2.0]));
        assert!(approx(r.offset, [-0.5, -0.5]));
        assert!(!r.repeat);
    }

    #[test]
    fn tile_repeats_from_origin() {
        let r = UvRect::compute(Fit::Tile, (100, 50), (200, 100));
        assert!(approx(r.scale, [2.0, 2.0]));
        assert!(approx(r.offset, [0.0, 0.0]));
        assert!(r.repeat);
    }

    #[test]
    fn zero_sizes_do_not_divide_by_zero() {
        let r = UvRect::compute(Fit::Fill, (0, 0), (0, 0));
        assert!(r.scale[0].is_finite() && r.scale[1].is_finite());
    }
}
