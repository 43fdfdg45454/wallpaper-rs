use super::SceneView;
use super::item::scale_of;
use crate::Renderer;
use crate::texture::create_image;
use ab_glyph::{Font, FontArc, FontVec, PxScale, ScaleFont, point};
use image::{Rgba, RgbaImage};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, OnceLock};
use wprs_scene::{Affine, Align, Content, Frame};

const MAX_EDGE: u32 = 4096;

pub(crate) struct Fonts {
    packaged: HashMap<String, FontArc>,
    system: HashMap<String, Option<FontArc>>,
}

fn database() -> &'static fontdb::Database {
    static DB: OnceLock<fontdb::Database> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    })
}

fn system(name: &str) -> Option<FontArc> {
    let db = database();
    let family = match name {
        "" | "sans-serif" => fontdb::Family::SansSerif,
        "serif" => fontdb::Family::Serif,
        "monospace" => fontdb::Family::Monospace,
        name => fontdb::Family::Name(name),
    };
    let query = fontdb::Query { families: &[family, fontdb::Family::SansSerif], ..Default::default() };
    let id = db.query(&query).or_else(|| db.faces().next().map(|f| f.id))?;
    db.with_face_data(id, |data, index| FontVec::try_from_vec_and_index(data.to_vec(), index).ok())?
        .map(FontArc::new)
}

impl Fonts {
    pub(crate) fn new(files: &HashMap<String, Arc<Vec<u8>>>) -> Fonts {
        let load = |(id, data): (&String, &Arc<Vec<u8>>)| {
            Some((id.clone(), FontArc::new(FontVec::try_from_vec(data.to_vec()).ok()?)))
        };
        Fonts { packaged: files.iter().filter_map(load).collect(), system: HashMap::new() }
    }

    pub(crate) fn get(&mut self, name: Option<&str>) -> Option<FontArc> {
        let name = name.unwrap_or_default();
        if let Some(font) = self.packaged.get(name) {
            return Some(font.clone());
        }
        self.system.entry(name.to_owned()).or_insert_with(|| system(name)).clone()
    }
}

pub(crate) fn raster(font: &FontArc, text: &str, px: f32, align: Align) -> Option<RgbaImage> {
    let scaled = font.as_scaled(PxScale::from(px.clamp(1.0, 1024.0)));
    let line_height = scaled.height() + scaled.line_gap();
    let rows = (MAX_EDGE as f32 / line_height.max(1.0)).ceil() as usize;
    let lines: Vec<(f32, Vec<(ab_glyph::GlyphId, f32)>)> = text
        .lines()
        .take(rows)
        .map(|line| {
            let (mut x, mut last, mut glyphs) = (0.0, None, Vec::new());
            for c in line.chars() {
                if x > MAX_EDGE as f32 {
                    break;
                }
                let id = scaled.glyph_id(c);
                x += last.map_or(0.0, |l| scaled.kern(l, id));
                glyphs.push((id, x));
                x += scaled.h_advance(id);
                last = Some(id);
            }
            (x, glyphs)
        })
        .collect();
    let width = (lines.iter().map(|(w, _)| *w).fold(0.0, f32::max).ceil() as u32 + 2).min(MAX_EDGE);
    let height = ((line_height * lines.len().max(1) as f32).ceil() as u32 + 2).min(MAX_EDGE);
    if text.trim().is_empty() {
        return None;
    }
    let mut image = RgbaImage::from_pixel(width, height, Rgba([255, 255, 255, 0]));
    for (row, (line_width, glyphs)) in lines.iter().enumerate() {
        let shift = match align {
            Align::Left => 0.0,
            Align::Center => (width as f32 - 2.0 - line_width) / 2.0,
            Align::Right => width as f32 - 2.0 - line_width,
        };
        let baseline = 1.0 + scaled.ascent() + row as f32 * line_height;
        for (id, x) in glyphs {
            let glyph = id.with_scale_and_position(scaled.scale(), point(1.0 + shift + x, baseline));
            let Some(outline) = font.outline_glyph(glyph) else { continue };
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
            let Content::Text { text, font, size, align, .. } = &d.content else { continue };
            let k = scale_of(screen.then(d.world)).max(1e-3);
            let px = (size * k * 2.0).round() / 2.0;
            let key = {
                let mut h = DefaultHasher::new();
                (text, font, px.to_bits(), *align as u8).hash(&mut h);
                h.finish()
            };
            let frame_no = view.pool.frame;
            if let Some(entry) = view.texts.get_mut(&key) {
                entry.2 = frame_no;
            } else if let Some(image) = view.fonts.get(*font).and_then(|f| raster(&f, text, px, *align)) {
                let (w, h) = image.dimensions();
                let gpu = create_image(&self.device, &self.queue, image.as_raw(), w, h, w * 4);
                view.pool.created += 1;
                view.texts.insert(key, (gpu, [w as f32 * size / px, h as f32 * size / px], frame_no));
            }
            if view.texts.contains_key(&key) {
                found.push((i, key));
            }
        }
        found
    }
}
