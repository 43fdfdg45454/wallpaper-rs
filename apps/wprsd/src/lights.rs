use crate::compositor::Hdr;
use wprs_core::Config;
use wprs_core::model::HdrState;

pub(crate) fn state(config: &Config, output: &str, hdr: &Hdr) -> HdrState {
    let nits = |v: Option<f32>| v.map(|n| f64::from(n.round()));
    HdrState {
        choice: config.hdr(output).as_str().into(),
        own: config.own_hdr(output).map(|c| c.as_str().into()),
        active: hdr.active,
        offered: hdr.offered,
        display: hdr.display,
        peak_nits: nits(hdr.peak_nits),
        reference_nits: nits(hdr.reference_nits),
        reason: hdr.reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_tells_the_choice_and_what_the_display_does() {
        let mut config = Config::default();
        config.set_option("playback.hdr.DP-1", "on").unwrap();
        let hdr = Hdr {
            active: true,
            offered: true,
            display: true,
            peak_nits: Some(799.6),
            reference_nits: Some(203.0),
            reason: "on",
        };
        let shown = state(&config, "DP-1", &hdr);
        assert_eq!((shown.choice.as_str(), shown.own.as_deref()), ("on", Some("on")));
        assert_eq!((shown.peak_nits, shown.reference_nits, shown.active), (Some(800.0), Some(203.0), true));
        let plain = state(&config, "DP-2", &Hdr::default());
        assert_eq!(
            (plain.choice.as_str(), plain.own, plain.active, plain.peak_nits),
            ("auto", None, false, None)
        );
        assert!((hdr.headroom() - 799.6 / 203.0).abs() < 1e-4 && Hdr::default().headroom() == 1.0);
    }
}
