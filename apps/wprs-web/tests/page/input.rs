use super::helper::{Page, available, start};
use serde_json::json;
use std::time::{Duration, Instant};
use wprs_web::{Allowed, Isolation, ToWeb};

fn moved(page: &mut Page, at: (f64, f64)) {
    page.link.send(&ToWeb::Pointer { view: 1, at: Some(at) }, &[]).unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    let want = json!([at.0 as i64, at.1 as i64, true]);
    while Instant::now() < end && page.said.get("move").and_then(|m| m.last()) != Some(&want) {
        page.pump(Instant::now() + Duration::from_millis(20));
    }
    assert_eq!(page.said.get("move").and_then(|m| m.last()), Some(&want));
}

#[test]
fn a_real_click_reaches_the_page_once_and_only_when_allowed() {
    if !available() {
        return eprintln!("skipped: no WPE WebKit 2.54");
    }
    let dir = tempfile::tempdir().unwrap();
    let mut page =
        Page::open(start(dir.path(), 9), Isolation { network: true, devices: false, limits: false });
    assert_eq!(page.wait("visits", 20), "1");
    moved(&mut page, (40.0, 30.0));
    page.link.send(&ToWeb::Press { view: 1, at: (100.0, 50.0), button: 1 }, &[]).unwrap();
    assert_eq!(page.wait("click", 10), json!([100, 50, true]));
    moved(&mut page, (41.0, 31.0));
    assert_eq!(page.said["click"].len(), 1, "{:?}", page.said["click"]);
    let allowed = Allowed { audio: true, pointer: true, clicks: false };
    page.link.send(&ToWeb::Allowed(allowed), &[]).unwrap();
    page.link.send(&ToWeb::Press { view: 1, at: (10.0, 10.0), button: 1 }, &[]).unwrap();
    moved(&mut page, (20.0, 20.0));
    assert_eq!(page.said["click"].len(), 1, "a click arrived while clicks are off");
    assert_eq!(page.crashed, 0);
    assert!(page.child.try_wait().unwrap().is_none(), "the helper stopped");
}
