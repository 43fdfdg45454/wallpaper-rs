use super::*;
use crate::hdr::{Screen, headroom, report, screen, wanted};

pub(super) fn hdr_follows_the_setting_and_the_display() {
    assert_eq!(report()["choice"], "auto");
    assert_eq!(settings::setting("playback.hdr", Some("on")).unwrap(), "on");
    assert_eq!(report()["choice"], "on");
    assert!(settings::setting("playback.hdr", Some("bright")).is_err());
    assert_eq!(settings::chosen("playback.hdr"), "on");
    settings::setting("playback.hdr", Some("auto")).unwrap();
    screen(true, 1000.0, 3.5);
    assert_eq!((report()["display"].clone(), report()["peak_nits"].clone()), (json!(true), json!(1000.0)));
    screen(false, f32::NAN, -2.0);
    assert_eq!((report()["display"].clone(), report()["peak_nits"].clone()), (json!(false), json!(0.0)));
    screen(true, 1e9, 1e9);
    assert_eq!(report()["peak_nits"], json!(100_000.0));
    assert_eq!(report()["active"], json!(false), "nothing was drawn in HDR");
    let at = |hdr, peak_nits, ratio| headroom(Screen { hdr, peak_nits, ratio });
    assert_eq!(at(true, 1000.0, 3.0), 3.0);
    assert!((at(true, 812.0, 0.0) - 4.0).abs() < 1e-3);
    assert_eq!(at(false, 0.0, 0.0), wprs_core::hdr::UNKNOWN_HEADROOM);
    assert_eq!(at(true, 0.0, 0.5), wprs_core::hdr::UNKNOWN_HEADROOM);
    screen(false, 0.0, 0.0);
    assert_eq!(wanted(true, true, true), (false, "display"), "an SDR screen stays SDR at auto");
    screen(true, 0.0, 0.0);
    assert_eq!(wanted(true, true, true), (true, "auto"));
    assert_eq!(wanted(true, true, false), (false, "quality"));
    settings::setting("playback.hdr", Some("on")).unwrap();
    screen(false, 0.0, 0.0);
    assert_eq!(wanted(true, false, false), (true, "on"));
    settings::setting("playback.hdr", Some("auto")).unwrap();
}
