use super::*;

#[test]
fn round_trip_from_keygen_to_revocation() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.add(DUSK, "2.1.0");
    let published = repo.publish(NOW);
    assert_eq!((published.version, published.packages, published.shards), (1, 2, 16));
    let info = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap();
    assert_eq!((info.root_version, info.entry_version, info.packages, info.stale), (1, 1, 2, false));
    assert_eq!(info.name, "Example wallpapers");
    assert!(info.warnings.is_empty(), "{:?}", info.warnings);
    assert!(matches!(add_repo(&repo.data, &FileFetch, &repo.url(), NOW), Err(Error::Exists(_))));
    let all: Vec<String> = browse(&repo.data, "").unwrap().into_iter().map(|i| i.id).collect();
    assert_eq!(all, [DAWN, DUSK]);
    let found = browse(&repo.data, "DUSK").unwrap();
    assert_eq!((found.len(), found[0].release.version.as_str()), (1, "2.1.0"));
    let done = install(&repo.data, &FileFetch, DAWN, NOW, false, false).unwrap();
    assert_eq!(done.path, repo.data.join("packages").join(DAWN).join("1.0.0.wpak"));
    assert_eq!(done.signer.as_deref(), Some(repo.author.fingerprint.as_str()));
    assert_eq!(
        fs::read(&done.path).unwrap(),
        fs::read(repo.dir.join("packages").join(DAWN).join("1.0.0.wpak")).unwrap()
    );
    assert_eq!((mode(&done.path), mode(done.path.parent().unwrap())), (0o600, 0o700));
    assert_eq!(files(done.path.parent().unwrap()), ["1.0.0.wpak"]);
    assert_eq!(browse(&repo.data, "dawn").unwrap()[0].installed, ["1.0.0"]);
    revoke(&repo.dir, DAWN, Some("1.0.0"), Reason::Malware, "Contains a hidden payload.").unwrap();
    assert_eq!(repo.publish(NOW + 60).version, 2);
    let outcome = update_repo(&repo.data, &FileFetch, &info.repo_id, NOW + 120).unwrap();
    assert_eq!(outcome.repo.entry_version, 2);
    assert_eq!(
        outcome.revoked.iter().map(|i| (i.id.as_str(), i.revoked)).collect::<Vec<_>>(),
        [(DAWN, Some(Reason::Malware))]
    );
    assert_eq!(installed(&repo.data).unwrap()[0].revoked, Some(Reason::Malware));
    assert!(browse(&repo.data, "dawn").unwrap().is_empty());
    assert!(matches!(
        install(&repo.data, &FileFetch, "org.example.dawn@1.0.0", NOW, false, false),
        Err(Error::Revoked(_))
    ));
    assert_eq!(repos(&repo.data, NOW).unwrap()[0].entry_version, 2);
    remove_repo(&repo.data, &info.repo_id).unwrap();
    assert!(repos(&repo.data, NOW).unwrap().is_empty());
}

#[test]
fn versions_updates_and_removal() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.publish(NOW);
    let repo_id = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap().repo_id;
    install(&repo.data, &FileFetch, DAWN, NOW, false, false).unwrap();
    repo.add(DAWN, "1.1.0");
    repo.publish(NOW + 10);
    let outcome = update_repo(&repo.data, &FileFetch, &repo_id, NOW + 20).unwrap();
    let updates: Vec<_> =
        outcome.updates.iter().map(|u| (u.installed.as_str(), u.available.as_str())).collect();
    assert_eq!(updates, [("1.0.0", "1.1.0")]);
    assert_eq!(install(&repo.data, &FileFetch, DAWN, NOW + 20, false, false).unwrap().version, "1.1.0");
    assert_eq!(install(&repo.data, &FileFetch, DAWN, NOW + 20, false, false).unwrap().version, "1.1.0");
    assert!(update_repo(&repo.data, &FileFetch, &repo_id, NOW + 30).unwrap().updates.is_empty());
    let spec = format!("{repo_id}/{DAWN}@1.0.0");
    let older = install(&repo.data, &FileFetch, &spec, NOW + 20, false, false);
    assert!(matches!(&older, Err(Error::Invalid { reason, .. }) if reason.contains("older")), "{older:?}");
    assert_eq!(install(&repo.data, &FileFetch, &spec, NOW + 20, false, true).unwrap().version, "1.0.0");
    assert_eq!(install(&repo.data, &FileFetch, &spec, NOW + 20, false, true).unwrap().version, "1.0.0");
    assert_eq!(remove(&repo.data, DAWN, Some("1.0.0")).unwrap().len(), 1);
    assert_eq!(files(&repo.data.join("packages").join(DAWN)), ["1.1.0.wpak"]);
    assert_eq!(remove(&repo.data, DAWN, None).unwrap()[0].version, "1.1.0");
    assert!(!repo.data.join("packages").join(DAWN).exists());
    assert!(matches!(remove(&repo.data, DAWN, None), Err(Error::NotFound(_))));
}

#[test]
fn packages_fall_back_from_bad_mirrors() {
    let repo = Repo::with(|tmp, s| s.mirrors = vec![file_url(&tmp.join("repo"))]);
    repo.add(DAWN, "1.0.0");
    repo.add(DUSK, "1.0.0");
    repo.publish(NOW);
    let bad = repo.copy("bad");
    let longer = bad.join("packages").join(DAWN).join("1.0.0.wpak");
    fs::write(&longer, [fs::read(&longer).unwrap(), b"tail".to_vec()].concat()).unwrap();
    let flipped = bad.join("packages").join(DUSK).join("1.0.0.wpak");
    let mut bytes = fs::read(&flipped).unwrap();
    bytes[40] ^= 1;
    fs::write(&flipped, bytes).unwrap();
    add_repo(&repo.data, &FileFetch, &repo.url_at(&bad), NOW).unwrap();
    install(&repo.data, &FileFetch, DAWN, NOW, false, false).unwrap();
    install(&repo.data, &FileFetch, DUSK, NOW, false, false).unwrap();
    let alone = Repo::new();
    alone.add(DAWN, "1.0.0");
    alone.publish(NOW);
    let path = alone.dir.join("packages").join(DAWN).join("1.0.0.wpak");
    let mut bytes = fs::read(&path).unwrap();
    bytes[40] ^= 1;
    fs::write(&path, bytes).unwrap();
    add_repo(&alone.data, &FileFetch, &alone.url(), NOW).unwrap();
    assert!(matches!(install(&alone.data, &FileFetch, DAWN, NOW, false, false), Err(Error::Mismatch(_))));
    assert!(files(&alone.data.join("packages").join(DAWN)).is_empty());
}

#[test]
fn manifests_must_match_their_listing() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    let catalog = repo.dir.join("catalog.json");
    fs::write(&catalog, fs::read_to_string(&catalog).unwrap().replace("CC0-1.0", "CC-BY-4.0")).unwrap();
    repo.publish(NOW);
    add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap();
    assert!(matches!(install(&repo.data, &FileFetch, DAWN, NOW, false, false), Err(Error::Mismatch(_))));
}
