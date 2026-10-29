use super::SceneView;
use super::item::scale_of;
use crate::Renderer;
use crate::texture::create_image;
use ab_glyph::{Font, FontArc, GlyphId, PxScale, ScaleFont, point};
use image::{Rgba, RgbaImage};
use std::hash::{DefaultHasher, Hash, Hasher};
use wprs_scene::{Affine, Align, Content, Frame, Layout};

const MAX_EDGE: u32 = 4096;
const ELLIPSIS: char = '…';

#[derive(Debug, Clone, Copy)]
struct Placed {
    font: usize,
    id: GlyphId,
    x: f32,
}

#[derive(Debug, Default)]
struct Line {
    width: f32,
    glyphs: Vec<Placed>,
}

struct Typesetter<'f> {
    fonts: &'f [FontArc],
    scale: PxScale,
}

fn breaks_after(c: char) -> bool {
    let wide = matches!(u32::from(c), 0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF);
    c.is_whitespace() || c == '-' || wide || u32::from(c) >= 0x20000
}

impl Typesetter<'_> {
    fn pick(&self, c: char) -> (usize, GlyphId) {
        let found = self.fonts.iter().enumerate().map(|(i, f)| (i, f.glyph_id(c))).find(|(_, id)| id.0 != 0);
        found.unwrap_or((0, self.fonts[0].glyph_id(c)))
    }

    fn advance(&self, (font, id): (usize, GlyphId)) -> f32 {
        self.fonts[font].as_scaled(self.scale).h_advance(id)
    }

    fn line(&self, chars: &[char]) -> Line {
        let (mut line, mut pen, mut last) = (Line::default(), 0.0, None::<(usize, GlyphId)>);
        for c in chars {
            let (font, id) = self.pick(*c);
            let scaled = self.fonts[font].as_scaled(self.scale);
            pen += last.filter(|l| l.0 == font).map_or(0.0, |l| scaled.kern(l.1, id));
            line.glyphs.push(Placed { font, id, x: pen });
            pen += scaled.h_advance(id);
            if !c.is_whitespace() {
                line.width = pen;
            }
            last = Some((font, id));
        }
        line
    }

    fn wrap<'c>(&self, chars: &'c [char], width: Option<f32>) -> Vec<&'c [char]> {
        let (mut lines, mut start, mut x, mut cut) = (Vec::new(), 0, 0.0, None::<usize>);
        for (i, c) in chars.iter().enumerate() {
            if *c == '\n' {
                lines.push(&chars[start..i]);
                (start, x, cut) = (i + 1, 0.0, None);
                continue;
            }
            let advance = self.advance(self.pick(*c));
            if width.is_some_and(|w| x + advance > w) && i > start && !c.is_whitespace() {
                let at = cut.filter(|b| *b > start).unwrap_or(i);
                lines.push(&chars[start..at]);
                (start, cut) = (at, None);
                x = chars[at..i].iter().map(|c| self.advance(self.pick(*c))).sum();
            }
            x += advance;
            if breaks_after(*c) {
                cut = Some(i + 1);
            }
        }
        lines.push(&chars[start..]);
        lines
    }

    fn ellipsis(&self, chars: &[char], width: f32) -> Line {
        let mut kept = chars.to_vec();
        loop {
            while kept.last().is_some_and(|c| c.is_whitespace()) {
                kept.pop();
            }
            let line = self.line(&[&kept[..], &[ELLIPSIS]].concat());
            if line.width <= width || kept.is_empty() {
                return line;
            }
            kept.pop();
        }
    }
}

pub(crate) fn raster(
    fonts: &[FontArc],
    text: &str,
    px: f32,
    align: Align,
    k: f32,
    layout: &Layout,
) -> Option<RgbaImage> {
    let scale = PxScale::from(px.clamp(1.0, 1024.0));
    let first = fonts.first()?.as_scaled(scale);
    let set = Typesetter { fonts, scale };
    let advance = layout.line_height.map_or(first.height() + first.line_gap(), |h| h * px);
    let limit = layout.wrap.map(|w| w.width * k);
    let chars: Vec<char> = text.chars().take(16_384).collect();
    let pieces = set.wrap(&chars, limit);
    let rows = ((MAX_EDGE as f32 / advance.max(1.0)).ceil() as usize)
        .min(layout.wrap.and_then(|w| w.lines).map_or(usize::MAX, |n| n as usize));
    let cut = pieces.len() > rows;
    let mut lines: Vec<Line> = pieces.iter().take(rows).map(|p| set.line(p)).collect();
    if cut && layout.wrap.is_some_and(|w| w.ellipsis) {
        let last = pieces[rows - 1];
        lines[rows - 1] = set.ellipsis(last, limit.unwrap_or(f32::MAX));
    }
    if text.trim().is_empty() {
        return None;
    }
    let block =
        limit.unwrap_or_else(|| lines.iter().map(|l| l.width).fold(0.0, f32::max)).min(MAX_EDGE as f32);
    let [bx, by] = layout.backing.map_or([0.0; 2], |(_, padding, _)| padding.map(|p| p * k));
    let margin = layout.padding * k + 1.0;
    let width = ((block + 2.0 * (bx + margin)).ceil() as u32).min(MAX_EDGE);
    let height = ((advance * lines.len().max(1) as f32 + 2.0 * (by + margin)).ceil() as u32).min(MAX_EDGE);
    let mut image = RgbaImage::from_pixel(width, height, Rgba([255, 255, 255, 0]));
    for (row, line) in lines.iter().enumerate() {
        let shift = match align {
            Align::Left => 0.0,
            Align::Center => (block - line.width) / 2.0,
            Align::Right => block - line.width,
        };
        let baseline = margin + by + first.ascent() + row as f32 * advance;
        for g in &line.glyphs {
            let glyph = g.id.with_scale_and_position(scale, point(margin + bx + shift + g.x, baseline));
            let Some(outline) = fonts[g.font].outline_glyph(glyph) else { continue };
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, c| {
                let (x, y) = (bounds.min.x as i32 + gx as i32, bounds.min.y as i32 + gy as i32);
                if x >= 0
                    && y >= 0
                    && let Some(p) = image.get_pixel_mut_checked(x as u32, y as u32)
                {
                    p.0[3] = p.0[3].max((c.clamp(0.0, 1.0) * 255.0).round() as u8);
                }
            });
        }
    }
    Some(image)
}

impl Renderer {
    pub(crate) fn rasters(
        &self,
        view: &mut SceneView,
        frame: &Frame<'_>,
        screen: Affine,
    ) -> Vec<(usize, u64)> {
        let mut found = Vec::new();
        for (i, d) in frame.draws.iter().enumerate() {
            let Content::Text { text, font, size, align, fallback, layout, .. } = &d.content else {
                continue;
            };
            let k = scale_of(screen.then(d.world)).max(1e-3);
            let px = (size * k * 2.0).round() / 2.0;
            let key = {
                let mut h = DefaultHasher::new();
                let wrap = layout.wrap.map(|w| (w.width.to_bits(), w.lines, w.ellipsis));
                let backing = layout.backing.map(|b| (b.1.map(f32::to_bits), b.2.to_bits()));
                let spacing = (layout.line_height.map(f32::to_bits), layout.padding.to_bits(), backing);
                (text, font, fallback, px.to_bits(), *align as u8, wrap, spacing).hash(&mut h);
                h.finish()
            };
            let frame_no = view.pool.frame;
            if let Some(entry) = view.texts.get_mut(&key) {
                entry.2 = frame_no;
            } else {
                let fonts = view.fonts.chain(*font, fallback);
                if let Some(image) = raster(&fonts, text, px, *align, px / size, layout) {
                    let (w, h) = image.dimensions();
                    let gpu = create_image(&self.device, &self.queue, image.as_raw(), w, h, w * 4);
                    view.pool.created += 1;
                    view.texts.insert(key, (gpu, [w as f32 * size / px, h as f32 * size / px], frame_no));
                }
            }
            if view.texts.contains_key(&key) {
                found.push((i, key));
            }
        }
        found
    }
}
