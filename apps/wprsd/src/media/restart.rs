use super::{Listener, Wake};
use crate::compositor::lock;
use crate::confirm::tests::{connect, private_bus};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

async fn until(what: impl Fn() -> bool) -> bool {
    for _ in 0..200 {
        if what() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    false
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_listener_that_fails_waits_and_is_started_again() {
    let bus = private_bus().expect("dbus-daemon is installed");
    let connection = connect(&bus.1).await;
    let told = Arc::new(AtomicUsize::new(0));
    let counted = told.clone();
    let changed: Wake = Arc::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
    });
    let l = Listener::new(connection, Arc::new(|| {}), changed, Duration::from_millis(300));
    l.follow(true);
    let first = lock(&l.task).as_ref().map(|(id, _)| *id).unwrap();
    assert!(until(|| lock(&l.status).listening).await, "the players were never read");
    assert_eq!(lock(&l.status).players, 0);
    drop(bus);
    assert!(until(|| !lock(&l.status).listening).await, "a lost bus still reads as listening");
    let before = told.load(Ordering::SeqCst);
    assert!(until(|| lock(&l.task).is_none()).await, "the failed listener never ended");
    assert!(told.load(Ordering::SeqCst) > before, "the end of the listener was not told");
    l.follow(true);
    let second = lock(&l.task).as_ref().map(|(id, _)| *id).unwrap();
    assert_ne!(first, second, "a new listener starts after the failure");
    assert!(until(|| lock(&l.task).is_none()).await, "a listener that cannot read gives up again");
    l.follow(true);
    l.follow(false);
    assert!(lock(&l.task).is_none() && !lock(&l.status).listening);
}
