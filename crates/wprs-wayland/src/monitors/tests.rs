use super::*;

fn info(x: i16, width: u16) -> MonitorInfo {
    let (height, width_in_millimeters, height_in_millimeters) = (720, 340, 190);
    MonitorInfo { x, width, height, width_in_millimeters, height_in_millimeters, ..Default::default() }
}

fn named(name: &str, x: i32) -> Monitor {
    let area = Rect { x, y: 0, width: 640, height: 720 };
    Monitor { name: name.into(), area, mm: (340, 190), refresh: 0 }
}

#[test]
fn monitors_keep_their_randr_names_and_skip_empty_areas() {
    let none = HashMap::new();
    assert_eq!(monitor(0, Some("DP-1".into()), &info(0, 640), &none), Some(named("DP-1", 0)));
    assert_eq!(monitor(1, None, &info(640, 640), &none), Some(named("X11-1", 640)));
    assert_eq!(monitor(2, Some(String::new()), &info(640, 640), &none), Some(named("X11-2", 640)));
    assert_eq!(monitor(0, Some("DP-2".into()), &info(0, 0), &none), None);
}

#[test]
fn monitors_take_the_fastest_mode_of_their_outputs() {
    let mode = |dot_clock, htotal, vtotal, flags: u32| ModeInfo {
        dot_clock,
        htotal,
        vtotal,
        mode_flags: flags.into(),
        ..Default::default()
    };
    let interlaced = u32::from(ModeFlag::INTERLACE);
    assert_eq!(millihertz(&mode(148_500_000, 2200, 1125, 0)), 60_000);
    assert_eq!(millihertz(&mode(586_590_000, 2720, 1497, 0)), 144_060);
    assert_eq!(millihertz(&mode(74_250_000, 2200, 1125, interlaced)), 60_053);
    assert_eq!(millihertz(&mode(0, 0, 0, 0)), 0);
    let rates = HashMap::from([(7, 59_940), (8, 170_000)]);
    let fast = MonitorInfo { outputs: vec![7, 8], ..info(0, 640) };
    assert_eq!(monitor(0, Some("DP-1".into()), &fast, &rates).map(|m| m.refresh), Some(170_000));
}

#[test]
fn changes_list_removed_names_then_new_or_moved_monitors() {
    let old = [named("DP-1", 0), named("DP-2", 640)];
    assert_eq!(changes(&old, &old), (vec![], vec![]));
    let new = [named("DP-1", 640), named("HDMI-1", 0)];
    assert_eq!(changes(&old, &new), (vec!["DP-2".into()], new.to_vec()));
    assert_eq!(changes(&[], &old), (vec![], old.to_vec()));
    assert_eq!(changes(&old, &[]), (vec!["DP-1".into(), "DP-2".into()], vec![]));
}
