use super::*;

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
    assert_eq!(repos(&repo.data, NOW).unwrap()[0].root_version, 1);
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
