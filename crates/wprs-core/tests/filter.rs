use wprs_core::Config;
use wprs_core::filter::{Filter, LOOKS, Look};

#[test]
fn filters_are_checked_and_kept_per_output() {
    let warm = Filter { look: Look::Warm, strength: 0.5, ..Filter::default() };
    let mut c = Config::default();
    c.set_filter("DP-1", warm);
    c.set_filter("HDMI-1", Filter::default());
    assert_eq!((c.filter("DP-1"), c.filter("eDP-1")), (warm, Filter::default()));
    c.set_filter("", Filter { look: Look::Mono, ..Filter::default() });
    assert!(c.outputs.values().all(|o| o.filter.is_none()) && c.filter("DP-1").look == Look::Mono);
    c.set_filter("", Filter::default());
    assert!(c.default.filter.is_none());
    let text = "[outputs.DP-1.filter]\nlook = \"sepia\"\nexposure = 9.0\nhue = -30.0\n";
    let mut read: Config = toml::from_str(text).unwrap();
    assert_eq!(read.normalize(), ["outputs.DP-1.filter"]);
    let kept = read.filter("DP-1");
    assert_eq!((kept.look, kept.exposure, kept.hue), (Look::Sepia, 3.0, -30.0));
    assert!(toml::from_str::<Config>("[default.filter]\nlook = \"teal\"\n").is_err());
    assert!(toml::from_str::<Config>("[default.filter]\nblur = 1.0\n").is_err());
}

#[test]
fn filter_values_have_ranges_and_looks_have_names() {
    let mut f = Filter::default();
    assert!(
        f.set("hue", 181.0).is_err() && f.set("contrast", f32::NAN).is_err() && f.set("blur", 0.0).is_err()
    );
    f.set("exposure", -3.0).unwrap();
    assert!(f.valid() && !f.is_identity());
    assert!(!Filter { saturation: 2.0, ..Filter::default() }.valid());
    assert!(Filter { look: Look::Noir, strength: 0.0, ..Filter::default() }.is_identity());
    assert!(
        LOOKS.iter().all(|l| Look::parse(l.name()) == Some(*l)) && Look::parse("Warm") == Some(Look::Warm)
    );
    let grey = [0.2f32; 3];
    let mono = Filter { look: Look::Mono, ..Filter::default() }.grade().unwrap().apply([0.4, 0.1, 0.05]);
    assert!((mono[0] - mono[1]).abs() < 1e-4 && (mono[1] - mono[2]).abs() < 1e-4);
    let darker = Filter { exposure: -1.0, ..Filter::default() }.grade().unwrap().apply(grey);
    assert!((darker[0] - 0.1).abs() < 1e-3, "{darker:?}");
    let warm = Filter { look: Look::Warm, ..Filter::default() }.grade().unwrap().apply(grey);
    assert!(warm[0] > grey[0] && warm[2] < grey[2]);
}
