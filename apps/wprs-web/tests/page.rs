#[path = "page/folders.rs"]
mod folders;
#[path = "page/helper.rs"]
mod helper;
#[path = "page/input.rs"]
mod input;
#[path = "page/media.rs"]
mod media;
#[path = "page/reload.rs"]
mod reload;

use helper::{Page, available, start};
use serde_json::json;
use std::net::TcpListener;
use std::path::Path;
use std::time::{Duration, Instant};
use wprs_web::{Isolation, Start, ToWeb};

#[test]
fn page_gets_its_api_and_nothing_else() {
    if !available() {
        return eprintln!("skipped: no WPE WebKit 2.54");
    }
    let dir = tempfile::tempdir().unwrap();
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    server.set_nonblocking(true).unwrap();
    let port = server.local_addr().unwrap().port();
    let mut page =
        Page::open(start(dir.path(), port), Isolation { network: true, devices: false, limits: false });
    assert_eq!(page.wait("early", 20), "preloaded");
    assert_eq!(page.wait("language", 5), "es-AR");
    assert_eq!(page.wait("properties", 5)["tint"]["hex"], "#ff8000");
    assert_eq!(page.wait("settings", 5)["fps"], 20);
    for name in ["manifest", "other", "file", "escape", "user", "local"] {
        assert_eq!(page.wait(name, 10), "blocked", "{name}");
    }
    assert_eq!(page.wait("own", 5).as_str().map(|s| s.starts_with("read")), Some(true));
    assert_eq!(page.wait("picked", 5), "read 8");
    assert_eq!(page.wait("files", 5).as_array().map(Vec::len), Some(1));
    assert_eq!(page.wait("visits", 5), "1");
    assert!(server.accept().is_err(), "the page reached a local server");
    let values = json!({"speed": 2}).as_object().unwrap().clone();
    page.link.send(&ToWeb::Properties { view: 1, values }, &[]).unwrap();
    page.said.remove("properties");
    assert_eq!(page.wait("properties", 5), json!({"speed": 2}));
    page.said.remove("frames");
    page.wait("frames", 5);
    let rate = page.wait("frames", 5).as_u64().unwrap();
    assert!((10..=26).contains(&rate), "{rate} frames a second at a cap of 20");
    page.link.send(&ToWeb::Visible { view: 1, visible: false }, &[]).unwrap();
    assert_eq!(page.wait("paused", 5), true);
    page.frames = 0;
    page.pump(Instant::now() + Duration::from_secs(2));
    assert!(page.frames <= 2, "{} frames while hidden", page.frames);
    let required = std::env::var_os("WPRS_REQUIRE_WEB").is_some_and(|v| v == "1");
    assert!(!required || wprs_web::isolated(page.child.id()), "the page shares the network of the daemon");
}

#[test]
fn page_without_a_render_node_draws_in_memory() {
    if !available() {
        return eprintln!("skipped: no WPE WebKit 2.54");
    }
    let dir = tempfile::tempdir().unwrap();
    let start = Start { device: None, storage: None, mask_gpu: true, ..start(dir.path(), 9) };
    let mut page = Page::open(start, Isolation { network: true, devices: true, limits: false });
    assert_eq!(page.wait("visits", 20), "1");
    let webgl = page.wait("webgl", 5);
    assert!(webgl.is_null() || webgl == "WebKit WebGL", "{webgl}");
    page.frames = 0;
    page.pump(Instant::now() + Duration::from_secs(1));
    assert!(page.frames >= 5, "{} frames in memory", page.frames);
}

#[test]
fn a_node_that_cannot_hold_a_buffer_is_not_given_to_the_page() {
    let helper = Path::new(helper::HELPER);
    assert_eq!(wprs_web::device(helper, None), None);
    assert_eq!(wprs_web::device(helper, Some("/dev/null".into())), None);
    assert_eq!(wprs_web::device(helper, Some("/dev/dri/renderD9999".into())), None);
}
