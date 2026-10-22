use crate::daemon::{Daemon, State};
use std::sync::Arc;

pub(crate) fn fit((w, h): (u32, u32), real: Option<(u32, u32)>) -> (u32, u32) {
    match real {
        Some((rw, rh)) if rw > 0 && rh > 0 && (w > rw || h > rh) => {
            let f = (f64::from(rw) / f64::from(w)).min(f64::from(rh) / f64::from(h));
            let scaled = |v: u32, max: u32| ((f64::from(v) * f - 1e-6).ceil() as u32).clamp(1, max);
            (scaled(w, rw), scaled(h, rh))
        }
        _ => (w, h),
    }
}

impl State {
    pub(crate) fn real_pixels(&self, output: &str) -> Option<(u32, u32)> {
        let screen = self.outputs.get(output).and_then(|info| info.screen.as_ref());
        let pixels = |l: u32, scale: f64| (f64::from(l) * scale).round() as u32;
        let screen = screen.map(|s| (pixels(s.width, s.scale), pixels(s.height, s.scale)));
        self.modes.get(output).copied().or(screen)
    }
}

pub(crate) fn follow_modes(daemon: &Arc<Daemon>) {
    let weak = Arc::downgrade(daemon);
    let watched = wprs_wayland::watch_modes(move |output, size| {
        let Some(daemon) = weak.upgrade() else { return };
        let mut st = daemon.state();
        match size {
            Some(size) => drop(st.modes.insert(output.clone(), size)),
            None => drop(st.modes.remove(&output)),
        }
        drop(st);
        if daemon.refit(&output) {
            daemon.configure_all();
        }
    });
    if let Err(e) = watched {
        tracing::debug!("streams keep the requested size, the screen sizes are unknown: {e:#}");
    }
}

impl Daemon {
    pub(crate) fn refit(&self, output: &str) -> bool {
        let mut st = self.state();
        let real = st.real_pixels(output);
        let streamed = self.exporter.stream_info(output).is_some();
        let Some(info) = st.outputs.get_mut(output).filter(|_| streamed) else { return false };
        let size = fit(info.requested, real);
        if size == info.size {
            return false;
        }
        tracing::info!(output, width = size.0, height = size.1, "stream resized to the pixels of the screen");
        info.size = size;
        st.resized.push((output.to_owned(), size));
        self.exporter.ensure_stream(output, size.0, size.1, Box::new(drop));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::fit;

    #[test]
    fn requests_shrink_to_the_pixels_of_the_screen_keeping_their_shape() {
        assert_eq!(fit((3414, 1920), Some((2560, 1440))), (2560, 1440));
        assert_eq!(fit((2561, 1440), Some((2560, 1440))), (2560, 1440));
        assert_eq!(fit((4000, 1000), Some((2560, 1440))), (2560, 640));
        assert_eq!(fit((1920, 1080), Some((2560, 1440))), (1920, 1080));
        assert_eq!(fit((3072, 1728), None), (3072, 1728));
        assert_eq!(fit((640, 360), Some((0, 0))), (640, 360));
    }
}
