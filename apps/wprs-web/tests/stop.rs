use std::path::Path;
use std::time::{Duration, Instant};
use wprs_web::{Isolation, end, members, spawn};

fn helper(script: &str) -> (wprs_web::Link, std::process::Child) {
    spawn(Path::new("/bin/sh"), Isolation::default(), |c| {
        c.args(["-c", script]);
    })
    .unwrap()
}

fn settled(group: u32) -> bool {
    let until = Instant::now() + Duration::from_secs(2);
    while !members(group).is_empty() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    members(group).is_empty()
}

#[test]
fn a_helper_that_quits_ends_without_being_killed() {
    let (link, child) = helper("sleep 30 & cat <&3 >/dev/null; kill $!; wait");
    let group = child.id();
    let until = Instant::now() + Duration::from_secs(2);
    while members(group).len() < 2 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(members(group).len() >= 2);
    drop(link);
    let started = Instant::now();
    assert!(end(child, Duration::from_secs(5), Duration::from_secs(5)));
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(members(group).is_empty());
}

#[test]
fn a_helper_that_stays_is_killed_with_everything_it_started() {
    let (_link, child) = helper("trap '' TERM; sleep 30 & sleep 30; wait");
    let group = child.id();
    let started = Instant::now();
    assert!(!end(child, Duration::from_millis(100), Duration::from_millis(100)));
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(settled(group));
}

#[test]
fn processes_a_quit_helper_leaves_behind_are_stopped() {
    let (link, child) = helper("sleep 30 & cat <&3 >/dev/null");
    let group = child.id();
    drop(link);
    assert!(!end(child, Duration::from_secs(5), Duration::from_millis(100)));
    assert!(settled(group));
}
