use super::*;

const MARKS: [u32; 4] = [1, 2, 3, 4];

fn client(x: i32, width: u32, shown: bool, full: bool) -> Client {
    Client { area: Rect { x, y: 0, width, height: 720 }, shown, full, ..Client::default() }
}

#[test]
fn fullscreen_or_fully_maximized_windows_are_full_and_hidden_ones_are_marked() {
    assert_eq!(status(&[9, 1], MARKS), (true, false));
    assert_eq!(status(&[2, 3], MARKS), (true, false));
    assert_eq!(status(&[2], MARKS), (false, false));
    assert_eq!(status(&[3, 4], MARKS), (false, true));
    assert_eq!(status(&[], MARKS), (false, false));
}

#[test]
fn only_mapped_windows_on_the_current_desktop_are_shown() {
    assert!(shown(true, false, Some(1), Some(1)));
    assert!(shown(true, false, Some(EVERY_DESKTOP), Some(2)));
    assert!(shown(true, false, None, Some(2)) && shown(true, false, Some(3), None));
    assert!(!shown(true, false, Some(1), Some(2)));
    assert!(!shown(false, false, Some(1), Some(1)) && !shown(true, true, None, None));
}

#[test]
fn a_monitor_is_covered_by_a_full_window_or_by_windows_that_tile_it() {
    let left = Rect { x: 0, y: 0, width: 640, height: 720 };
    let right = Rect { x: 640, ..left };
    let clients = [client(0, 640, true, true), client(640, 640, false, true), client(700, 300, true, false)];
    assert!(covered(left, &clients) && !covered(right, &clients));
    assert!(covered(right, &[client(400, 1000, true, true)]));
    assert!(!covered(left, &[client(400, 1000, true, true)]));
    assert!(covered(right, &[client(640, 320, true, false), client(960, 320, true, false)]));
    assert!(!covered(right, &[client(640, 320, true, false), client(960, 320, false, false)]));
    assert!(!covered(left, &[]));
}

#[test]
fn desktop_icon_windows_cover_when_opaque_and_stay_above_ours() {
    let screen = Rect { x: 0, y: 0, width: 640, height: 720 };
    let icons = |opaque| Client { window: 7, desktop: true, opaque, ..client(0, 640, true, false) };
    assert!(covered(screen, &[icons(true)]) && !covered(screen, &[icons(false)]));
    let ours = Client { window: 3, ..client(0, 640, true, false) };
    assert_eq!(above(&[3], &[icons(true), ours]), [(3, 7)]);
    assert!(above(&[3], &[ours, icons(true)]).is_empty());
}

#[test]
fn a_larger_or_smaller_root_paints_the_root_background_again() {
    let screens = || [(5, "DP-1".to_owned()), (6, "DP-2".to_owned())].into_iter();
    let names = |events: Vec<Event>| -> Vec<String> {
        let name = |e: Event| match e {
            Event::Backdrop(n) => format!("backdrop {n}"),
            Event::Exposed(n) => format!("exposed {n}"),
            _ => String::new(),
        };
        events.into_iter().map(name).collect()
    };
    assert!(notices(screens(), &HashSet::new(), false).is_empty());
    assert_eq!(names(notices(screens(), &HashSet::from([6]), false)), ["exposed DP-2"]);
    assert_eq!(names(notices(screens(), &HashSet::new(), true)), ["backdrop DP-1", "backdrop DP-2"]);
}
