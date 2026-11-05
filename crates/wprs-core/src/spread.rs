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

fn named(name: &str) -> bool {
    (1..=128).contains(&name.len()) && !name.chars().any(char::is_control)
}

impl Layout {
    pub const FOLLOW: &str = "layout.follow.";

    pub fn follow(&mut self, output: &str, value: &str) -> Result<(), String> {
        let (output, value) = (output.trim(), value.trim());
        named(output)
            .then_some(())
            .ok_or_else(|| format!("invalid output name in {}<output>", Self::FOLLOW))?;
        if matches!(value, "" | "none") {
            self.follow.remove(output);
            return Ok(());
        }
        if !named(value) || value == output {
            return Err(format!("{value:?} is not another output for {}{output}", Self::FOLLOW));
        }
        if !self.follow.contains_key(output) && self.follow.len() >= Self::MAX_MONITORS {
            return Err(format!("at most {} outputs can follow another", Self::MAX_MONITORS));
        }
        self.follow.insert(output.into(), value.into());
        Ok(())
    }

    pub(crate) fn tidy(&mut self) {
        self.follow.retain(|output, leader| named(output) && named(leader) && output != leader);
        while self.follow.len() > Self::MAX_MONITORS {
            self.follow.pop_last();
        }
    }

    pub fn leader<'a>(&'a self, output: &'a str) -> &'a str {
        self.follow.get(output).map_or(output, String::as_str)
    }

    pub fn loose<'a>(&self, outputs: impl IntoIterator<Item = &'a str>) -> Vec<Monitor> {
        let fine = |mm: &[f64; 4]| mm.iter().all(|v| v.is_finite()) && mm[2] > 0.0 && mm[3] > 0.0;
        let placed = |id: &str| self.monitors.get(id).map(|p| p.mm).filter(fine);
        outputs.into_iter().filter_map(|id| Some(Monitor { id: id.into(), mm: placed(id)? })).collect()
    }
}
