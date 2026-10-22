mod fixture;

use fixture::{Counting, DAWN, DAY, DUSK, NOW, Repo, file_url, files, key, signer};
use std::fs;
use std::path::Path;
use wprs_format::{
    Error, FileFetch, add_repo, comment, publish, repos, rotate_root, settings, sign_root, status,
    update_repo,
};

fn resign(repo: &Repo, path: &str, edit: impl FnOnce(&mut serde_json::Value)) {
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(repo.dir.join(path)).unwrap()).unwrap();
    edit(&mut value);
    let bytes = serde_json::to_vec(&value).unwrap();
    let version = value["version"].as_u64().unwrap();
    let sig =
        signer(&repo.online).sign(&bytes, &comment("entry", version, &status(&repo.dir).unwrap().repo_id));
    fs::write(repo.dir.join(path), &bytes).unwrap();
    fs::write(repo.dir.join(format!("{path}.minisig")), sig.unwrap()).unwrap();
}

fn state(data: &Path) -> Vec<u8> {
    let dir = fs::read_dir(data.join("repos")).unwrap().next().unwrap().unwrap().path();
    fs::read(dir.join("state.json")).unwrap()
}

#[test]
fn anchors_must_match() {
    let repo = Repo::new();
    repo.publish(NOW);
    let (base, id) = (file_url(&repo.dir), status(&repo.dir).unwrap().repo_id);
    let wrong_id = format!("{base}?repo_id={}", "0".repeat(64));
    assert!(matches!(add_repo(&repo.data, &FileFetch, &wrong_id, NOW), Err(Error::Mismatch(_))));
    let wrong_key = format!("{base}?fingerprint={}", repo.online.fingerprint);
    assert!(matches!(add_repo(&repo.data, &FileFetch, &wrong_key, NOW), Err(Error::Signature(_))));
    assert!(matches!(add_repo(&repo.data, &FileFetch, &base, NOW), Err(Error::Invalid { .. })));
    assert!(repos(&repo.data).unwrap().is_empty());
    let both = format!("{base}?fingerprint={}&repo_id={id}", repo.root.fingerprint);
    assert_eq!(add_repo(&repo.data, &FileFetch, &both, NOW).unwrap().repo_id, id);
}

#[test]
fn rollback_and_equivocation_are_rejected() {
    let repo = Repo::with(|tmp, s| s.mirrors = vec![file_url(&tmp.join("repo"))]);
    repo.publish(NOW);
    repo.publish(NOW + 10);
    let old = repo.copy("old");
    let id = add_repo(&repo.data, &FileFetch, &repo.url_at(&old), NOW + 20).unwrap().repo_id;
    fs::write(old.join("entry/latest"), "1\n").unwrap();
    ["2.json", "2.json.minisig"].iter().for_each(|f| fs::remove_file(old.join("entry").join(f)).unwrap());
    repo.publish(NOW + 30);
    assert_eq!(update_repo(&repo.data, &FileFetch, &id, NOW + 40).unwrap().repo.entry_version, 3);
    let lone = Repo::new();
    lone.publish(NOW);
    lone.publish(NOW + 10);
    let id = add_repo(&lone.data, &FileFetch, &lone.url(), NOW + 20).unwrap().repo_id;
    let before = state(&lone.data);
    resign(&lone, "entry/2.json", |v| v["expires"] = "2027-01-16T08:00:00Z".into());
    assert!(matches!(update_repo(&lone.data, &FileFetch, &id, NOW + 30), Err(Error::Equivocation(2))));
    for file in ["1.json", "1.json.minisig"] {
        fs::copy(lone.dir.join("entry").join(file), lone.dir.join("entry").join(file.replace('1', "2")))
            .unwrap();
    }
    fs::write(lone.dir.join("entry/latest"), "1\n").unwrap();
    assert!(update_repo(&lone.data, &FileFetch, &id, NOW + 30).is_err());
    assert_eq!(state(&lone.data), before);
}

#[test]
fn timestamps_are_bounded_by_the_clock() {
    let repo = Repo::new();
    repo.publish(NOW + 2 * DAY);
    let result = add_repo(&repo.data, &FileFetch, &repo.url(), NOW);
    assert!(matches!(&result, Err(Error::Invalid { field, .. }) if field == "entry.timestamp"), "{result:?}");
    let id = add_repo(&repo.data, &FileFetch, &repo.url(), NOW + 2 * DAY).unwrap().repo_id;
    let outcome = update_repo(&repo.data, &FileFetch, &id, NOW - 365 * DAY).unwrap();
    assert!(!outcome.repo.stale);
    assert!(matches!(publish(&repo.dir, &signer(&repo.online), 7, NOW), Err(Error::Invalid { .. })));
}

#[test]
fn entries_need_the_online_key_and_a_short_life() {
    let repo = Repo::new();
    assert!(matches!(publish(&repo.dir, &signer(&repo.online), 30, NOW), Err(Error::Invalid { .. })));
    repo.publish(NOW);
    let id = status(&repo.dir).unwrap().repo_id;
    let entry = fs::read(repo.dir.join("entry/1.json")).unwrap();
    let forged = signer(&repo.root).sign(&entry, &comment("entry", 1, &id)).unwrap();
    fs::write(repo.dir.join("entry/1.json.minisig"), forged).unwrap();
    assert!(matches!(add_repo(&repo.data, &FileFetch, &repo.url(), NOW), Err(Error::Signature(_))));
    resign(&repo, "entry/1.json", |v| v["expires"] = "2027-02-15T08:00:00Z".into());
    let result = add_repo(&repo.data, &FileFetch, &repo.url(), NOW);
    assert!(matches!(&result, Err(Error::Invalid { field, .. }) if field == "entry.expires"), "{result:?}");
}

#[test]
fn latest_pointer_is_probed_and_capped() {
    let repo = Repo::new();
    repo.publish(NOW);
    repo.publish(NOW + 1);
    fs::write(repo.dir.join("entry/latest"), "1\n").unwrap();
    let info = add_repo(&repo.data, &FileFetch, &repo.url(), NOW + 2).unwrap();
    assert_eq!(info.entry_version, 2);
    fs::write(repo.dir.join("entry/latest"), "1".repeat(1 << 20)).unwrap();
    let result = update_repo(&repo.data, &FileFetch, &info.repo_id, NOW + 3);
    assert!(matches!(result, Err(Error::Limit { what: "download size", .. })), "{result:?}");
}

#[test]
fn shards_are_cached_and_verified() {
    let repo = Repo::with(|tmp, s| s.mirrors = vec![file_url(&tmp.join("repo"))]);
    repo.add(DAWN, "1.0.0");
    repo.publish(NOW);
    let fetch = Counting::default();
    let id = add_repo(&repo.data, &fetch, &repo.url(), NOW).unwrap().repo_id;
    assert_eq!((fetch.count("index/"), fetch.count("revoked/")), (16, 1));
    fetch.0.borrow_mut().clear();
    update_repo(&repo.data, &fetch, &id, NOW + 1).unwrap();
    assert_eq!(fetch.count("index/") + fetch.count("revoked/"), 0);
    repo.add(DUSK, "1.0.0");
    repo.publish(NOW + 2);
    update_repo(&repo.data, &fetch, &id, NOW + 3).unwrap();
    assert_eq!(fetch.count("index/"), 1);
    assert_eq!(files(&repo.data.join("repos").join(&id).join("blobs")).len(), 17);
    let bad = repo.copy("bad");
    for (n, name) in files(&bad.join("index")).iter().enumerate() {
        let path = bad.join("index").join(name);
        let text = fs::read_to_string(&path).unwrap();
        fs::write(&path, if n % 2 == 0 { text.replace("shard", "shart") } else { text + " " }).unwrap();
    }
    let other = repo.tmp.path().join("other");
    assert_eq!(add_repo(&other, &FileFetch, &repo.url_at(&bad), NOW + 4).unwrap().packages, 2);
    let lone = Repo::new();
    lone.publish(NOW);
    let shard = lone.dir.join("index").join(&files(&lone.dir.join("index"))[0]);
    fs::write(&shard, fs::read_to_string(&shard).unwrap() + "      ").unwrap();
    let result = add_repo(&lone.data, &FileFetch, &lone.url(), NOW);
    assert!(matches!(result, Err(Error::Limit { .. }) | Err(Error::Mismatch(_))), "{result:?}");
}

#[test]
fn root_rotation_follows_the_chain() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.publish(NOW);
    let id = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap().repo_id;
    let (root2, online2) = (key(), key());
    let mut next = settings(&repo.dir).unwrap();
    (next.root_keys, next.online_keys) = (vec![root2.public.clone()], vec![online2.public.clone()]);
    let rotated = rotate_root(&repo.dir, &next, NOW + 10).unwrap();
    assert_eq!((rotated.version, rotated.complete), (2, false));
    assert!(!sign_root(&repo.dir, &signer(&root2)).unwrap().complete);
    assert!(matches!(update_repo(&repo.data, &FileFetch, &id, NOW + 20), Err(Error::Signature(_))));
    assert_eq!(repos(&repo.data).unwrap()[0].root_version, 1);
    let root2_json = fs::read(repo.dir.join("root/2.json")).unwrap();
    let old_only = signer(&repo.root).sign(&root2_json, &comment("root", 2, &id)).unwrap();
    fs::write(repo.dir.join("root/2.json.minisig"), old_only).unwrap();
    assert!(!status(&repo.dir).unwrap().complete);
    assert!(matches!(update_repo(&repo.data, &FileFetch, &id, NOW + 20), Err(Error::Signature(_))));
    assert!(sign_root(&repo.dir, &signer(&root2)).unwrap().complete);
    assert!(matches!(publish(&repo.dir, &signer(&repo.online), 7, NOW + 30), Err(Error::Invalid { .. })));
    publish(&repo.dir, &signer(&online2), 7, NOW + 30).unwrap();
    let outcome = update_repo(&repo.data, &FileFetch, &id, NOW + 40).unwrap();
    assert_eq!((outcome.repo.root_version, outcome.repo.entry_version, outcome.repo.packages), (2, 2, 1));
    let fresh = repo.tmp.path().join("fresh");
    assert_eq!(add_repo(&fresh, &FileFetch, &repo.url(), NOW + 40).unwrap().root_version, 2);
}

#[test]
fn root_thresholds_count_distinct_keys() {
    let (b, c) = (key(), key());
    let repo = Repo::with(|_, s| {
        s.root_keys.extend([b.public.clone(), c.public.clone()]);
        s.root_threshold = 2;
    });
    let sig = repo.dir.join("root/1.json.minisig");
    fs::write(&sig, fs::read_to_string(&sig).unwrap().repeat(2)).unwrap();
    assert!(!status(&repo.dir).unwrap().complete);
    assert!(matches!(publish(&repo.dir, &signer(&repo.online), 7, NOW), Err(Error::Invalid { .. })));
    assert!(sign_root(&repo.dir, &signer(&c)).unwrap().complete);
    repo.publish(NOW);
    assert_eq!(add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap().root_version, 1);
}

#[test]
fn expired_roots_abort_updates() {
    let repo = Repo::with(|_, s| s.root_days = 30);
    repo.publish(NOW);
    let id = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap().repo_id;
    assert!(matches!(update_repo(&repo.data, &FileFetch, &id, NOW + 31 * DAY), Err(Error::Expired(_))));
}

#[test]
fn hostile_mirrors_leave_state_untouched() {
    let repo = Repo::with(|tmp, s| s.mirrors = vec![file_url(&tmp.join("mirror"))]);
    repo.publish(NOW);
    let id = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap().repo_id;
    let before = state(&repo.data);
    repo.publish(NOW + 10);
    fs::write(repo.dir.join("entry/2.json.minisig"), signer(&repo.online).sign(b"other", "x").unwrap())
        .unwrap();
    repo.copy("mirror");
    assert!(matches!(update_repo(&repo.data, &FileFetch, &id, NOW + 20), Err(Error::Signature(_))));
    assert_eq!(state(&repo.data), before);
}
