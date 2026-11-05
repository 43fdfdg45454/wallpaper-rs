use crate::daemon::Daemon;
use crate::streams::OutputInfo;
use wprs_core::{Fit, Place};

impl OutputInfo {
    pub(crate) fn shown_fit(&self, fit: Fit) -> Fit {
        if self.spread { Fit::Span } else { self.uploaded.as_ref().map_or(fit, |u| u.1) }
    }
}

impl Daemon {
    pub(crate) fn scene_span(&self, output: &str, fit: Fit, canvas: [u32; 2]) -> Option<[f32; 4]> {
        let follows = self.state().config.layout.follow.contains_key(output);
        if fit != Fit::Span || (crate::drafts::virtual_output(output) && !follows) {
            return None;
        }
        let Place::Span { monitors, output: lead, .. } = self.place(output, Fit::Span, None) else {
            return None;
        };
        let parts = wprs_core::spread::rects(&monitors, canvas);
        parts.into_iter().find(|(id, _)| *id == lead).map(|(_, rect)| rect)
    }

    pub(crate) fn spreading(&self) -> Vec<String> {
        let st = self.state();
        st.outputs.iter().filter(|(_, info)| info.spread).map(|(name, _)| name.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use wprs_core::layout::Monitor;

    #[test]
    fn a_spread_scene_shows_on_each_screen_the_part_under_it() {
        let monitors = [
            Monitor { id: "left".into(), mm: [0.0, 0.0, 600.0, 340.0] },
            Monitor { id: "right".into(), mm: [620.0, 0.0, 600.0, 340.0] },
        ];
        let parts = wprs_core::spread::rects(&monitors, [3840, 1080]);
        let [l, r] = [&parts[0].1, &parts[1].1];
        assert!((l[2] - r[2]).abs() < 1e-3 && l[0] < r[0], "{parts:?}");
        assert!((r[0] - (l[0] + l[2])) > 0.0, "the frames between the screens hide a strip: {parts:?}");
        assert!((r[0] + r[2] - 3840.0).abs() < 1.0 || l[0] > 0.0, "{parts:?}");
    }
}
