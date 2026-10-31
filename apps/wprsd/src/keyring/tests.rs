use super::*;
use mock::{Shared, Vault, serve};
use std::sync::Mutex;

mod mock;

use mock::Bus;

fn unavailable_with(result: Result<()>) -> String {
    match result {
        Err(Error::Named(KEYRING_UNAVAILABLE, message)) => message,
        other => panic!("expected KeyringUnavailable, got {:?}", other.err()),
    }
}

async fn secret(keyring: &Keyring, id: &str, prompt: bool) -> Option<String> {
    match keyring.read(id, prompt).await.unwrap() {
        Stored::Secret(text) => Some(text.to_string()),
        Stored::Missing => None,
        Stored::Locked => Some("<locked>".into()),
    }
}

#[tokio::test]
async fn secrets_are_stored_read_and_deleted() {
    let bus = Bus::start();
    let keyring = Keyring::new(bus.connect().await);
    assert_eq!(unavailable_with(keyring.store("photos", "s3cret").await), ABSENT);
    assert!(!keyring.has("photos").await.unwrap() && secret(&keyring, "photos", true).await.is_none());
    keyring.delete("photos").await.unwrap();
    let vault = Shared::default();
    let _service = serve(&bus, &vault).await;
    for (id, value) in [("photos", "first"), ("photos", "s3cret"), ("album", "other")] {
        keyring.store(id, value).await.unwrap();
    }
    {
        let vault = vault.lock().unwrap();
        let labels: Vec<&str> = vault.items.iter().map(|item| item.3.as_str()).collect();
        assert_eq!(labels, ["wallpaper-rs: photos", "wallpaper-rs: album"]);
        assert_eq!(vault.items[0].1["xdg:schema"], SCHEMA);
    }
    assert!(keyring.has("photos").await.unwrap());
    assert_eq!(secret(&keyring, "photos", false).await.as_deref(), Some("s3cret"));
    keyring.delete("photos").await.unwrap();
    assert!(!keyring.has("photos").await.unwrap() && keyring.has("album").await.unwrap());
}

#[tokio::test]
async fn locked_keyrings_prompt_only_when_allowed() {
    let bus = Bus::start();
    let keyring = Keyring::new(bus.connect().await);
    let vault = Shared::new(Mutex::new(Vault { locked: true, ..Vault::default() }));
    let _service = serve(&bus, &vault).await;
    let refused = unavailable_with(keyring.store("photos", "s3cret").await);
    assert_eq!(refused, "unlock the keyring and try again");
    assert!(vault.lock().unwrap().items.is_empty());
    vault.lock().unwrap().accept = true;
    keyring.store("photos", "s3cret").await.unwrap();
    vault.lock().unwrap().locked = true;
    assert!(keyring.has("photos").await.unwrap());
    assert_eq!(secret(&keyring, "photos", false).await.as_deref(), Some("<locked>"));
    assert_eq!(secret(&keyring, "photos", true).await.as_deref(), Some("s3cret"));
    vault.lock().unwrap().locked = true;
    keyring.delete("photos").await.unwrap();
    assert!(vault.lock().unwrap().items.is_empty());
}

#[tokio::test]
async fn orphan_secrets_go_only_without_a_prompt() {
    let bus = Bus::start();
    let keyring = Keyring::new(bus.connect().await);
    assert!(!keyring.discard("photos").await.unwrap() && !keyring.reachable("photos").await);
    let vault = Shared::default();
    let _service = serve(&bus, &vault).await;
    keyring.store("photos", "old").await.unwrap();
    vault.lock().unwrap().locked = true;
    assert!(!keyring.discard("photos").await.unwrap());
    assert_eq!(vault.lock().unwrap().items.len(), 1);
    vault.lock().unwrap().locked = false;
    assert!(keyring.discard("photos").await.unwrap());
    assert!(vault.lock().unwrap().items.is_empty());
    assert!(keyring.reachable("photos").await);
}

#[tokio::test]
async fn a_keyring_that_does_not_answer_times_out() {
    let bus = Bus::start();
    let vault = Shared::new(Mutex::new(Vault { hang: true, ..Vault::default() }));
    let _service = serve(&bus, &vault).await;
    let (lookup, write) = (Duration::from_millis(200), Duration::from_millis(300));
    let keyring = Keyring { lookup, write, ..Keyring::new(bus.connect().await) };
    let started = std::time::Instant::now();
    assert_eq!(unavailable_with(keyring.has("photos").await.map(drop)), SLOW);
    assert_eq!(unavailable_with(keyring.read("photos", false).await.map(drop)), SLOW);
    assert_eq!(unavailable_with(keyring.store("photos", "s3cret").await), SLOW);
    assert_eq!(unavailable_with(keyring.delete("photos").await), SLOW);
    assert_eq!(unavailable_with(keyring.discard("photos").await.map(drop)), SLOW);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn after_a_lookup_times_out_the_next_ones_fail_at_once_and_answers_are_remembered() {
    let bus = Bus::start();
    let vault = Shared::default();
    let _service = serve(&bus, &vault).await;
    let lookup = Duration::from_millis(300);
    let keyring = Keyring { lookup, ..Keyring::new(bus.connect().await) };
    keyring.store("photos", "s3cret").await.unwrap();
    assert_eq!((keyring.known("photos"), keyring.known("album")), (Some(true), None));
    vault.lock().unwrap().hang = true;
    assert_eq!(unavailable_with(keyring.has("album").await.map(drop)), SLOW);
    let started = std::time::Instant::now();
    assert_eq!(unavailable_with(keyring.has("photos").await.map(drop)), SLOW);
    assert_eq!(unavailable_with(keyring.read("photos", false).await.map(drop)), SLOW);
    assert_eq!(unavailable_with(keyring.properties().has("photos").await.map(drop)), SLOW);
    assert!(started.elapsed() < Duration::from_millis(100));
    vault.lock().unwrap().hang = false;
    *keyring.silent.lock().unwrap() = None;
    assert!(!keyring.has("album").await.unwrap() && keyring.known("album") == Some(false));
    keyring.delete("photos").await.unwrap();
    assert_eq!(keyring.known("photos"), Some(false));
}
