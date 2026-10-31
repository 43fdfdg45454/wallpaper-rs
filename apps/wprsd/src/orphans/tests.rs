use super::Orphans;
use crate::keyring::Keyring;
use crate::keyring::tests::mock::{Bus, Shared, serve};

async fn kept(vault: &Shared) -> Vec<Vec<u8>> {
    vault.lock().unwrap().items.iter().map(|item| item.2.clone()).collect()
}

#[tokio::test]
async fn a_secret_stored_while_the_sweeper_waits_is_never_swept() {
    let (bus, dir) = (Bus::start(), tempfile::tempdir().unwrap());
    let keyring = Keyring::new(bus.connect().await);
    let vault = Shared::default();
    let _service = serve(&bus, &vault).await;
    keyring.store("dav", "old").await.unwrap();
    let orphans = Orphans::default();
    vault.lock().unwrap().locked = true;
    assert!(orphans.drop_one(&keyring, dir.path(), "dav").await);
    assert!(orphans.has("dav") && orphans.any());
    vault.lock().unwrap().locked = false;
    let held = orphans.writing.lock().await;
    let sweeping = orphans.sweep(&keyring, dir.path());
    let storing = async {
        assert!(orphans.mark(dir.path(), "dav", false));
        keyring.store("dav", "new").await.unwrap();
        drop(held);
    };
    let (cleared, ()) = tokio::join!(sweeping, storing);
    assert!(cleared.is_empty(), "the sweeper took a secret stored after it looked: {cleared:?}");
    assert_eq!(kept(&vault).await, [b"new".to_vec()]);
    let (stored, changed) = orphans.replace(&keyring, dir.path(), "dav", "newer").await;
    stored.unwrap();
    assert!(!changed && kept(&vault).await == [b"newer".to_vec()]);
    let reloaded = Orphans::default();
    reloaded.load(dir.path());
    assert!(!reloaded.has("dav"), "the cleared orphan came back from its file");
}

#[tokio::test]
async fn the_sweeper_deletes_an_orphan_once_the_keyring_opens_and_forgets_it() {
    let (bus, dir) = (Bus::start(), tempfile::tempdir().unwrap());
    let keyring = Keyring::new(bus.connect().await);
    let vault = Shared::default();
    let _service = serve(&bus, &vault).await;
    keyring.store("dav", "old").await.unwrap();
    let orphans = Orphans::default();
    vault.lock().unwrap().locked = true;
    assert!(orphans.mark(dir.path(), "dav", true) && !orphans.mark(dir.path(), "dav", true));
    assert!(orphans.sweep(&keyring, dir.path()).await.is_empty() && orphans.has("dav"));
    assert_eq!(kept(&vault).await.len(), 1, "a locked keyring lost the orphan");
    vault.lock().unwrap().locked = false;
    assert_eq!(orphans.sweep(&keyring, dir.path()).await, ["dav"]);
    assert!(kept(&vault).await.is_empty() && !orphans.any());
    let reloaded = Orphans::default();
    reloaded.load(dir.path());
    assert!(!reloaded.any());
}
