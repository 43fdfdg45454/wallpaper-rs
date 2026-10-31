use super::*;

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
    assert!(repos(&repo.data, NOW).unwrap().is_empty());
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
