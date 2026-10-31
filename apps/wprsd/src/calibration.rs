use crate::compositor::{Content, Picture, Spec};
use crate::daemon::{Daemon, State};
use image::{Rgba, RgbaImage};
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};
use wprs_core::layout::Monitor;
use wprs_core::{Fit, Place};

const SHOWN: Duration = Duration::from_secs(600);
const EDGE: f64 = 4096.0;
const SPACING: f64 = 100.0;
const LINE: f64 = 1.5;
const FRAME: f64 = 4.0;

#[derive(Default)]
pub(crate) struct Calibration {
    until: Option<Instant>,
    made: Option<(Vec<Monitor>, Arc<RgbaImage>, u64)>,
}

fn bounds(monitors: &[Monitor]) -> [f64; 4] {
    let lo = |i: usize| monitors.iter().map(|m| m.mm[i]).fold(f64::INFINITY, f64::min);
    let hi = |i: usize| monitors.iter().map(|m| m.mm[i] + m.mm[i + 2]).fold(f64::NEG_INFINITY, f64::max);
    [lo(0), lo(1), (hi(0) - lo(0)).max(1.0), (hi(1) - lo(1)).max(1.0)]
}

pub(crate) fn pattern(monitors: &[Monitor]) -> RgbaImage {
    let [x0, y0, w, h] = bounds(monitors);
    let px = (EDGE / w.max(h)).min(8.0);
    let (cx, cy) = (x0 + w / 2.0, y0 + h / 2.0);
    let line = |d: f64| (1.0 - (d.abs() - LINE / 2.0) * px).clamp(0.0, 1.0);
    let near = |v: f64| {
        let t = v.rem_euclid(SPACING);
        t.min(SPACING - t)
    };
    let size = |v: f64| ((v * px).round() as u32).max(1);
    RgbaImage::from_fn(size(w), size(h), |i, j| {
        let (x, y) = (x0 + (f64::from(i) + 0.5) / px, y0 + (f64::from(j) + 0.5) / px);
        let (dx, dy) = (x - cx, y - cy);
        let diagonals = line(near(dx + dy) / 2f64.sqrt()).max(line(near(dx - dy) / 2f64.sqrt()));
        let lit = diagonals.max(line(near(dx.hypot(dy)))).max(line(dx).max(line(dy)));
        let edge = monitors.iter().any(|m| {
            let [mx, my, mw, mh] = m.mm;
            let inside = x >= mx && y >= my && x <= mx + mw && y <= my + mh;
            inside && (x - mx).min(y - my).min(mx + mw - x).min(my + mh - y) < FRAME
        });
        let base = if edge { [230.0, 120.0, 20.0] } else { [24.0, 24.0, 24.0] };
        let [r, g, b] = base.map(|c: f64| (c + (255.0 - c) * lit).round() as u8);
        Rgba([r, g, b, 255])
    })
}

impl Daemon {
    pub fn show_calibration(self: &Arc<Self>, on: bool) {
        let started = {
            let calibration = &mut self.state().sidecars.calibration;
            let started = on && calibration.until.is_none();
            calibration.until = on.then(|| Instant::now() + SHOWN);
            started
        };
        if started {
            tokio::spawn(expire(Arc::downgrade(self)));
        }
        self.configure_all();
        self.changes.changed("state", Vec::new());
    }

    pub(crate) fn show_pattern(&self, output: &str, size: (u32, u32), refresh: u32) -> bool {
        let Some(picture) = self.calibration_for(output) else { return false };
        let (content, transition) = (Content::Image(picture), Some(Duration::ZERO));
        let spec = Spec { content, fit: Fit::Fill, background: [0; 3], transition, refresh };
        self.compositor.configure(output, size, spec);
        if let Some(info) = self.state().outputs.get_mut(output) {
            (info.uploaded, info.scene, info.video) = (None, None, None);
        }
        true
    }

    fn calibration_for(&self, output: &str) -> Option<Picture> {
        let monitors = {
            let st = self.state();
            st.sidecars.calibration.until?;
            monitors(&st)
        };
        let mine = monitors.iter().find(|m| m.id == output)?.mm;
        let kept = self.state().sidecars.calibration.made.clone().filter(|(m, ..)| *m == monitors);
        let (image, serial) = match kept {
            Some((_, image, serial)) => (image, serial),
            None => {
                let image = Arc::new(pattern(&monitors));
                let calibration = &mut self.state().sidecars.calibration;
                let serial = calibration.made.as_ref().map_or(0, |(.., s)| s + 1);
                calibration.made = Some((monitors.clone(), image.clone(), serial));
                (image, serial)
            }
        };
        let [x0, y0, w, h] = bounds(&monitors);
        let crop = [(mine[0] - x0) / w, (mine[1] - y0) / h, mine[2] / w, mine[3] / h].map(|v| v as f32);
        let file = PathBuf::from(format!("calibration-{serial}"));
        Some(Picture {
            file,
            modified: None,
            pixels: Some(image),
            edit: None,
            place: Place::Crop(crop),
            depth: None,
        })
    }
}

pub(crate) fn monitors(st: &State) -> Vec<Monitor> {
    let shown = st.outputs.iter().filter(|(name, _)| !crate::drafts::virtual_output(name));
    let screens = shown.filter_map(|(name, info)| Some((name.as_str(), info.size, info.screen.as_ref()?)));
    st.config.layout.arrange(screens)
}

pub(crate) fn calibrating(st: &State) -> bool {
    st.sidecars.calibration.until.is_some()
}

async fn expire(daemon: Weak<Daemon>) {
    while let Some(d) = daemon.upgrade() {
        let until = d.state().sidecars.calibration.until;
        match until {
            None => return,
            Some(until) if until <= Instant::now() => {
                d.state().sidecars.calibration = Calibration::default();
                d.changes.changed("state", Vec::new());
                return drop(tokio::task::spawn_blocking(move || d.configure_all()));
            }
            Some(until) => {
                drop(d);
                tokio::time::sleep_until(until.into()).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use wprs_core::layout::Monitor;

    #[test]
    fn the_pattern_spans_the_canvas_and_frames_every_screen() {
        let monitors = [
            Monitor { id: "A".into(), mm: [0.0, 0.0, 400.0, 200.0] },
            Monitor { id: "B".into(), mm: [410.0, 20.0, 300.0, 160.0] },
        ];
        let image = super::pattern(&monitors);
        let px: f64 = 4096.0 / 710.0;
        assert_eq!(image.dimensions(), (4096, (200.0 * px).round() as u32));
        assert_eq!(image.get_pixel(2, 100).0, [230, 120, 20, 255]);
        let center = image.get_pixel(2048, image.height() / 2).0;
        assert!(center[0] > 200, "{center:?}");
        assert_eq!(image.get_pixel(1000, 300).0, [24, 24, 24, 255]);
    }
}
