use crate::layout::{Layout, Monitor, span};

pub const LAYOUTS: [&str; 3] = ["dual", "triple", "vertical"];

const SCREEN_MM: [f64; 2] = [527.0, 296.0];

fn gap(bezel_mm: f64) -> f64 {
    2.0 * if bezel_mm.is_finite() { bezel_mm.clamp(0.0, Layout::MAX_BEZEL_MM) } else { 0.0 }
}

pub fn simulated(name: &str, bezel_mm: f64) -> Option<Vec<Monitor>> {
    let (count, vertical) = match name {
        "dual" => (2, false),
        "triple" => (3, false),
        "vertical" => (2, true),
        _ => return None,
    };
    let ([w, h], gap) = (SCREEN_MM, gap(bezel_mm));
    let place = |i: usize| {
        let step = i as f64;
        let mm = if vertical { [0.0, step * (h + gap), w, h] } else { [step * (w + gap), 0.0, w, h] };
        Monitor { id: format!("screen-{}", i + 1), mm }
    };
    Some((0..count).map(place).collect())
}

pub fn checked(monitors: Vec<Monitor>) -> Result<Vec<Monitor>, String> {
    if monitors.is_empty() || monitors.len() > Layout::MAX_MONITORS {
        return Err(format!("a layout has 1 to {} screens", Layout::MAX_MONITORS));
    }
    for m in &monitors {
        let named = (1..=256).contains(&m.id.len()) && !m.id.chars().any(char::is_control);
        let placed = m.mm[..2].iter().all(|v| v.is_finite() && v.abs() <= 100_000.0);
        let sized = m.mm[2..].iter().all(|v| v.is_finite() && *v > 0.0 && *v <= 10_000.0);
        if !(named && placed && sized) {
            return Err(format!("{:?} is not a screen with a name and a place in millimetres", m.id));
        }
    }
    Ok(monitors)
}

pub fn rects(monitors: &[Monitor], canvas: [u32; 2]) -> Vec<(String, [f32; 4])> {
    let [cw, ch] = canvas.map(|v| v.max(1) as f32);
    let parts = span(monitors, (canvas[0], canvas[1]), 1.0, [0.5, 0.5]);
    let rect = |o: [f32; 2], s: [f32; 2]| [o[0] * cw, o[1] * ch, s[0] * cw, s[1] * ch];
    parts.into_iter().map(|(id, uv)| (id, rect(uv.offset, uv.scale))).collect()
}

pub fn bounds(monitors: &[Monitor]) -> [f64; 4] {
    let lo = |i: usize| monitors.iter().map(|m| m.mm[i]).fold(f64::INFINITY, f64::min);
    let hi = |i: usize| monitors.iter().map(|m| m.mm[i] + m.mm[i + 2]).fold(f64::NEG_INFINITY, f64::max);
    if monitors.is_empty() {
        return [0.0, 0.0, 1.0, 1.0];
    }
    [lo(0), lo(1), (hi(0) - lo(0)).max(1e-3), (hi(1) - lo(1)).max(1e-3)]
}
