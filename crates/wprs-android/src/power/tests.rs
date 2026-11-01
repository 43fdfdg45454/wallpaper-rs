use super::*;

fn on(f: impl FnOnce(&mut Power)) -> Power {
    let mut power = Power::BATTERY;
    f(&mut power);
    power
}

#[test]
fn the_charger_doubles_the_cap_and_previews_stay_at_thirty() {
    let charging = on(|p| p.charging = true);
    assert_eq!((Power::BATTERY.fps(false), charging.fps(false), charging.fps(true)), (30, 60, 30));
}

#[test]
fn saving_power_or_a_low_battery_pauses_everything() {
    for power in [on(|p| p.saver = true), on(|p| p.low = true), on(|p| p.thermal = CRITICAL)] {
        assert_eq!((power.fps(false), power.moving()), (0, false));
    }
    assert_eq!(on(|p| (p.low, p.charging) = (true, true)).fps(false), 60);
}

#[test]
fn a_normal_battery_keeps_moving_and_each_pause_says_why() {
    assert_eq!((Power::BATTERY.moving(), Power::BATTERY.stills()), (true, None));
    let warm = on(|p| (p.thermal, p.headroom) = (MODERATE, 0.9));
    assert_eq!((warm.moving(), warm.stills(), warm.fps(false)), (true, None, 15));
    let reasons =
        [on(|p| p.saver = true), on(|p| p.low = true), on(|p| p.thermal = SEVERE)].map(Power::stills);
    assert_eq!(reasons, [Some("saver"), Some("battery"), Some("heat")]);
    assert_eq!(on(|p| (p.low, p.charging) = (true, true)).stills(), None);
}

#[test]
fn heat_lowers_the_rate_before_pausing() {
    assert_eq!(on(|p| p.headroom = 0.8).fps(false), 15);
    assert_eq!(on(|p| (p.thermal, p.charging) = (MODERATE, true)).fps(false), 30);
    let severe = on(|p| p.thermal = SEVERE);
    assert_eq!((severe.fps(false), severe.moving()), (10, false));
}

#[test]
fn the_chosen_caps_apply_before_heat() {
    let charging = on(|p| p.charging = true);
    assert_eq!((Power::BATTERY.capped(false, [15, 30]), charging.capped(true, [10, 60])), (15, 30));
    assert_eq!(on(|p| p.headroom = 0.8).capped(false, [10, 30]), 5);
}

#[test]
fn heat_lowers_the_scene_level_and_the_device_class_picks_the_tier() {
    let levels = [0, LIGHT, MODERATE, SEVERE].map(|t| on(|p| p.thermal = t).ceiling());
    assert_eq!(levels, [Quality::Q3, Quality::Q2, Quality::Q1, Quality::Q0]);
    assert_eq!(on(|p| (p.thermal, p.headroom) = (LIGHT, 0.9)).ceiling(), Quality::Q1);
    let tiers = [(false, 0), (false, 34), (true, 34)].map(|(low, class)| {
        device(low, class);
        tier(false, true)
    });
    assert_eq!(tiers, [Tier::T1, Tier::T2, Tier::T0]);
    device(false, 34);
    assert_eq!((tier(true, true), tier(false, false)), (Tier::T0, Tier::T0));
    device(false, 0);
}

#[test]
fn the_wait_leaves_room_for_the_next_vsync() {
    assert_eq!([60, 30, 10, 0].map(delay_ms), [12, 25, 92, 992]);
}
