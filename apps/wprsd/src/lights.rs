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
        encoding: hdr.encoding.into(),
        max_cll: hdr.described.map(|m| m.max_cll),
        max_fall: hdr.described.map(|m| m.max_fall),
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
            encoding: "pq-own",
            described: Some(wprs_wayland::Mastered { max_cll: 800, max_fall: 203 }),
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
        assert_eq!(
            (shown.encoding.as_str(), shown.max_cll, shown.max_fall),
            ("pq-own", Some(800), Some(203))
        );
        assert_eq!(hdr.mastered(), wprs_wayland::Mastered { max_cll: 800, max_fall: 203 });
        let unknown = Hdr { active: true, ..Hdr::default() };
        let guessed = wprs_core::hdr::UNKNOWN_HEADROOM * wprs_render::REFERENCE_NITS;
        assert_eq!(
            unknown.mastered().max_cll,
            guessed.round() as u32,
            "an unknown screen gets the documented guess"
        );
        assert_eq!(plain.encoding, "");
    }
}
