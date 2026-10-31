use super::helper::{Page, available, start, with};
use serde_json::json;
use std::time::{Duration, Instant};
use wprs_web::{Isolation, ToWeb};

fn web_processes(pid: u32) -> Vec<u32> {
    let children = |pid: u32| -> Vec<u32> {
        let tasks = std::fs::read_dir(format!("/proc/{pid}/task")).into_iter().flatten().flatten();
        let lists = tasks.filter_map(|t| std::fs::read_to_string(t.path().join("children")).ok());
        lists
            .flat_map(|l| l.split_whitespace().filter_map(|p| p.parse().ok()).collect::<Vec<u32>>())
            .collect()
    };
    let (mut all, mut todo) = (Vec::new(), children(pid));
    while let Some(pid) = todo.pop() {
        todo.extend(children(pid));
        all.push(pid);
    }
    let named = |pid: &u32| {
        std::fs::read_to_string(format!("/proc/{pid}/comm")).is_ok_and(|c| c.trim() == "WPEWebProcess")
    };
    all.into_iter().filter(named).collect()
}

fn crash(page: &mut Page) {
    let before = page.crashed;
    let pids = web_processes(page.child.id());
    assert!(!pids.is_empty(), "no web process");
    pids.iter().for_each(|pid| {
        unsafe { libc::kill(*pid as i32, libc::SIGKILL) };
    });
    let end = Instant::now() + Duration::from_secs(20);
    while Instant::now() < end && page.crashed == before {
        page.pump(Instant::now() + Duration::from_millis(20));
    }
    assert_eq!(page.crashed, before + 1);
}

fn reloaded(page: &mut Page) -> serde_json::Value {
    page.said.remove("properties");
    page.wait("properties", 30)
}

#[test]
fn a_crashed_page_loads_with_its_latest_values_and_gives_up_after_three_reloads() {
    if !available() {
        return eprintln!("skipped: no WPE WebKit 2.54");
    }
    let dir = tempfile::tempdir().unwrap();
    let mut page =
        Page::open(start(dir.path(), 9), Isolation { network: true, devices: false, limits: false });
    assert_eq!(page.wait("visits", 20), "1");
    let values = json!({"speed": 3}).as_object().unwrap().clone();
    page.link.send(&ToWeb::Properties { view: 1, values }, &[]).unwrap();
    page.said.remove("properties");
    assert_eq!(page.wait("properties", 5), json!({"speed": 3}));
    for _ in 0..3 {
        page.said.remove("properties");
        crash(&mut page);
        assert_eq!(reloaded(&mut page)["speed"], 3);
        assert!(!page.broken);
    }
    crash(&mut page);
    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end && !page.broken {
        page.pump(Instant::now() + Duration::from_millis(20));
    }
    assert!(page.broken, "the helper kept reloading a page that crashes");
}

#[test]
fn a_page_that_stops_answering_loads_again() {
    if !available() {
        return eprintln!("skipped: no WPE WebKit 2.54");
    }
    let dir = tempfile::tempdir().unwrap();
    let spin: &[u8] = b"console.log('probe load 1');\nsetTimeout(() => { for (;;) {} }, 1000);\n";
    let mut page = Page::open(
        with(dir.path(), 9, &[("probe.js", spin)]),
        Isolation { network: true, devices: false, limits: false },
    );
    page.wait("load", 20);
    let end = Instant::now() + Duration::from_secs(60);
    while Instant::now() < end && page.said["load"].len() < 2 {
        page.pump(Instant::now() + Duration::from_millis(50));
    }
    assert!(page.crashed >= 1, "the spinning page was never stopped");
    assert!(page.said["load"].len() >= 2, "the page did not load again");
}
