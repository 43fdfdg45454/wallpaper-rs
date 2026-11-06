use super::report::{report, written};
use super::*;
use std::sync::MutexGuard;

const SECOND: i64 = 1_000_000_000;
static SERIAL: Mutex<()> = Mutex::new(());

fn fresh() -> MutexGuard<'static, ()> {
    let guard = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    record(false);
    record(true);
    guard
}

fn shown(lock: bool, from: i64, seconds: i64, fps: i64, hz: f32) -> i64 {
    let stats = wprs_render::SceneStats { passes: 8, draws: 12, gpu_us: 2_500, ..Default::default() };
    let mut at = from;
    for i in 0..=seconds * fps {
        at = from + i * SECOND / fps;
        staged((1.0, 2.0), (Tier::T2, Quality::Q2, Why::Full), ((1080, 2400), (1080, 2400)), (60, 60), stats);
        presented(3.0, "fifo", 1);
        keep(usize::from(lock), at, (0.5, 7.0), (3, hz));
    }
    at
}

#[test]
fn the_report_counts_the_frames_of_the_last_thirty_seconds_and_says_how_long_each_part_took() {
    let _serial = fresh();
    let end = shown(false, 0, 40, 60, 60.0);
    let text = written(end);
    record(false);
    assert!(
        text.contains("1801 presented in 30.0 s shown of the last 30 s (1 runs), 60.0 per second"),
        "{text}"
    );
    assert!(text.contains("T2 Q2, full") && text.contains("fifo with latency 1"), "{text}");
    assert!(text.contains("draw cpu     mean 7.00") && text.contains("3.0 a frame"), "{text}");
    assert!(text.contains("encode     mean 2.00") && text.contains("8.0 passes, 12.0 draws"), "{text}");
    assert_eq!(report(), "performance recording is off");
}

#[test]
fn time_the_wallpaper_was_hidden_does_not_lower_the_rate() {
    let _serial = fresh();
    let before = shown(false, 0, 10, 60, 60.0);
    split(false);
    let end = shown(false, before + 12 * SECOND, 8, 60, 60.0);
    let text = written(end);
    record(false);
    assert!(
        text.contains("1082 presented in 18.0 s shown of the last 30 s (2 runs), 60.0 per second"),
        "{text}"
    );
    assert!(text.contains("interval     mean 16.67") && text.contains("max 16.67 ms"), "{text}");
}

#[test]
fn the_window_ends_now_and_not_at_the_last_frame() {
    let _serial = fresh();
    let end = shown(false, 0, 10, 60, 60.0);
    let text = written(end + 25 * SECOND);
    let gone = written(end + 31 * SECOND);
    record(false);
    assert!(text.contains("301 presented in 5.0 s shown of the last 30 s"), "{text}");
    assert!(gone.contains("none of a shown wallpaper in the last 30 s"), "{gone}");
}

#[test]
fn the_report_is_for_the_screen_shown_last_and_keeps_home_and_lock_apart() {
    let _serial = fresh();
    let lock = shown(true, 0, 5, 30, 120.0);
    let home = shown(false, lock + SECOND, 5, 60, 60.0);
    let text = written(home);
    split(true);
    let end = shown(true, home + SECOND, 5, 30, 120.0);
    let again = written(end);
    record(false);
    assert!(text.contains("screen       home, the one shown last"), "{text}");
    assert!(text.contains("301 presented in 5.0 s") && text.contains("60.0 to 60.0 Hz"), "{text}");
    assert!(again.contains("screen       lock") && again.contains("302 presented in 10.0 s"), "{again}");
    assert!(again.contains("(2 runs), 30.0 per second") && again.contains("120.0 to 120.0 Hz"), "{again}");
}

#[test]
fn the_kotlin_loop_counts_only_frames_kept_for_that_screen() {
    let _serial = fresh();
    let end = shown(false, 0, 1, 60, 90.0);
    looped(false, end, 4.0);
    looped(false, end - SECOND, 50.0);
    looped(true, end, 70.0);
    let text = written(end);
    record(false);
    assert!(text.contains("frame loop   mean 4.00") && text.contains("1 frames)"), "{text}");
}

#[test]
fn nothing_is_kept_for_previews_hidden_wallpapers_or_with_the_switch_off() {
    let _serial = fresh();
    assert!(measures(true, false, false));
    assert!(!measures(true, true, false) && !measures(true, false, true) && !measures(false, false, false));
    let mark = enter(measures(true, true, false));
    leave(mark, false, SECOND, true);
    let text = written(2 * SECOND);
    record(false);
    assert!(text.contains("none of a shown wallpaper"), "{text}");
}
