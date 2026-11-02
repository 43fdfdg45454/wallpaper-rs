use crate::{Device, STRIP};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Strip {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

impl Strip {
    pub fn new(width: u32, height: u32, rgb: Vec<u8>) -> Option<Strip> {
        let fits = width > 0 && height > 0 && width <= STRIP.0 && height <= STRIP.1;
        (fits && rgb.len() == (width * height * 3) as usize).then_some(Strip { width, height, rgb })
    }

    fn mean(&self, [x0, x1]: [f32; 2], [y0, y1]: [f32; 2]) -> [u8; 3] {
        let span = |a: f32, b: f32, n: u32| {
            let start = ((a * n as f32).floor() as u32).min(n - 1);
            (start, ((b * n as f32).ceil() as u32).clamp(start + 1, n))
        };
        let ((xa, xb), (ya, yb)) = (span(x0, x1, self.width), span(y0, y1, self.height));
        let mut sum = [0u32; 3];
        for y in ya..yb {
            for x in xa..xb {
                let at = ((y * self.width + x) * 3) as usize;
                (0..3).for_each(|c| sum[c] += u32::from(self.rgb[at + c]));
            }
        }
        let n = (xb - xa) * (yb - ya);
        sum.map(|s| ((s + n / 2) / n) as u8)
    }
}

pub fn shrink(width: u32, height: u32, pixels: &[u8], channels: u32) -> Option<Strip> {
    let wanted = u64::from(width) * u64::from(height) * u64::from(channels);
    if width == 0 || height == 0 || !(3..=4).contains(&channels) || pixels.len() as u64 != wanted {
        return None;
    }
    let scale = (STRIP.0 as f32 / width as f32).min(STRIP.1 as f32 / height as f32).min(1.0);
    let (w, h) =
        (((width as f32 * scale).round() as u32).max(1), ((height as f32 * scale).round() as u32).max(1));
    let full = Strip {
        width,
        height,
        rgb: pixels.chunks_exact(channels as usize).flat_map(|p| [p[0], p[1], p[2]]).collect(),
    };
    let cell = |i: u32, n: u32| [i as f32 / n as f32, (i + 1) as f32 / n as f32];
    let rgb =
        (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).flat_map(|(x, y)| full.mean(cell(x, w), cell(y, h)));
    Strip::new(w, h, rgb.collect())
}

pub fn colors(device: &Device, strip: &Strip) -> Vec<[u8; 3]> {
    let mut out = vec![[0u8; 3]; device.leds as usize];
    let line = |out: &mut Vec<[u8; 3]>, start: u32, n: u32| {
        for j in 0..n {
            let c = strip.mean([j as f32 / n as f32, (j + 1) as f32 / n as f32], [0.0, 1.0]);
            out.get_mut((start + j) as usize).into_iter().for_each(|slot| *slot = c);
        }
    };
    if device.zones.is_empty() {
        line(&mut out, 0, device.leds);
    }
    for zone in &device.zones {
        let Some((w, h, map)) = &zone.matrix else {
            line(&mut out, zone.start, zone.leds);
            continue;
        };
        for (k, led) in map.iter().enumerate().filter(|(_, led)| **led < zone.leds) {
            let (x, y) = (k as u32 % w, k as u32 / w);
            let c = strip.mean(
                [x as f32 / *w as f32, (x + 1) as f32 / *w as f32],
                [y as f32 / *h as f32, (y + 1) as f32 / *h as f32],
            );
            out.get_mut((zone.start + led) as usize).into_iter().for_each(|slot| *slot = c);
        }
    }
    out
}

pub fn pattern(phase: f32) -> Strip {
    let channel = |h: f32, n: f32| {
        let k = (n + h) % 6.0;
        (255.0 * (1.0 - k.min(4.0 - k).clamp(0.0, 1.0))) as u8
    };
    let rgb = (0..STRIP.0).flat_map(|x| {
        let h = (x as f32 / STRIP.0 as f32 + phase).fract() * 6.0;
        [channel(h, 5.0), channel(h, 3.0), channel(h, 1.0)]
    });
    Strip::new(STRIP.0, 1, rgb.collect()).unwrap_or_default()
}
