use image::DynamicImage;

const SIDE: u32 = 32;
const DISTINCT: u32 = 48;
const SHARE: usize = 20;
const BRIGHT: f32 = 0.7;
const DIM: f32 = 0.3;
const DARK_PIXEL: f32 = 0.225;
const DARK_AREA: f32 = 0.05;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    pub colors: Vec<u32>,
    pub dark_text: bool,
    pub dark_theme: bool,
}

fn linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn luminance([r, g, b]: [u8; 3]) -> f32 {
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

fn apart(a: [u32; 3], b: [u32; 3]) -> bool {
    a.iter().zip(b).map(|(x, y)| x.abs_diff(y).pow(2)).sum::<u32>() >= DISTINCT.pow(2)
}

pub fn palette(image: &DynamicImage) -> Palette {
    let small = image.thumbnail(SIDE, SIDE).to_rgb8();
    let mut bins = vec![(0usize, [0u32; 3]); 4096];
    let (mut light, mut dark) = (0.0, 0usize);
    for pixel in small.pixels() {
        let [r, g, b] = pixel.0.map(u32::from);
        let bin = &mut bins[(((r >> 4) << 8) | ((g >> 4) << 4) | (b >> 4)) as usize];
        bin.0 += 1;
        bin.1.iter_mut().zip([r, g, b]).for_each(|(sum, c)| *sum += c);
        let l = luminance(pixel.0);
        (light, dark) = (light + l, dark + usize::from(l <= DARK_PIXEL));
    }
    let mut ranked: Vec<_> = bins.into_iter().enumerate().filter(|(_, (n, _))| *n > 0).collect();
    ranked.sort_unstable_by_key(|&(i, (n, _))| std::cmp::Reverse((n, i)));
    let pixels = (small.width() * small.height()).max(1) as usize;
    let mut chosen: Vec<[u32; 3]> = Vec::with_capacity(3);
    for (_, (n, sum)) in ranked {
        let rgb = sum.map(|c| c / n as u32);
        if chosen.is_empty() || (n * SHARE >= pixels && chosen.iter().all(|c| apart(*c, rgb))) {
            chosen.push(rgb);
        }
        if chosen.len() == 3 {
            break;
        }
    }
    let (mean, dark) = (light / pixels as f32, dark as f32 / pixels as f32);
    Palette {
        colors: chosen.into_iter().map(|[r, g, b]| (r << 16) | (g << 8) | b).collect(),
        dark_text: mean > BRIGHT && dark <= DARK_AREA,
        dark_theme: mean < DIM,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn bands(rows: [(u32, [u8; 3]); 3]) -> DynamicImage {
        let mut image = RgbaImage::new(64, 64);
        let mut y = 0;
        for (height, [r, g, b]) in rows {
            (y..y + height).for_each(|y| (0..64).for_each(|x| image.put_pixel(x, y, Rgba([r, g, b, 255]))));
            y += height;
        }
        DynamicImage::ImageRgba8(image)
    }

    #[test]
    fn the_main_color_is_the_largest_bin() {
        let mut image = RgbaImage::from_pixel(64, 64, Rgba([0x33, 0x66, 0xff, 255]));
        (0..64).for_each(|x| image.put_pixel(x, 0, Rgba([255, 0, 0, 255])));
        assert_eq!(palette(&DynamicImage::ImageRgba8(image)).colors, [0x3366ff]);
    }

    #[test]
    fn secondary_colors_must_differ_and_cover_part_of_the_image() {
        let image = bands([(40, [20, 40, 160]), (16, [200, 120, 30]), (8, [36, 56, 176])]);
        assert_eq!(palette(&image).colors, [0x1428a0, 0xc8781e]);
    }

    #[test]
    fn hints_follow_the_brightness() {
        let light = palette(&bands([(64, [240, 240, 230]), (0, [0; 3]), (0, [0; 3])]));
        let dark = palette(&bands([(64, [10, 12, 20]), (0, [0; 3]), (0, [0; 3])]));
        let mixed = palette(&bands([(56, [250, 250, 250]), (8, [0, 0, 0]), (0, [0; 3])]));
        assert!(light.dark_text && !light.dark_theme);
        assert!(dark.dark_theme && !dark.dark_text);
        assert!(!mixed.dark_text && !mixed.dark_theme);
    }
}
