mod fixture;

use fixture::{DAWN, DAY, DUSK, NOW, Repo, file_url, files, key};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use wprs_format::{
    Anchor, Error, FileFetch, Reason, add, add_repo, browse, install, installed, publish, remove,
    remove_repo, repos, revoke, revoke_key, update_repo, verify,
};

fn mode(path: &std::path::Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

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
    assert!(matches!(add_repo(&repo.data, &FileFetch, &repo.url(), NOW), Err(Error::Invalid { .. })));
    let all: Vec<String> = browse(&repo.data, "").unwrap().into_iter().map(|i| i.id).collect();
    assert_eq!(all, [DAWN, DUSK]);
    let found = browse(&repo.data, "DUSK").unwrap();
    assert_eq!((found.len(), found[0].release.version.as_str()), (1, "2.1.0"));
    let done = install(&repo.data, &FileFetch, DAWN, NOW, false).unwrap();
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
        install(&repo.data, &FileFetch, "org.example.dawn@1.0.0", NOW, false),
        Err(Error::Revoked(_))
    ));
    assert_eq!(repos(&repo.data).unwrap()[0].entry_version, 2);
    remove_repo(&repo.data, &info.repo_id).unwrap();
    assert!(repos(&repo.data).unwrap().is_empty());
}

#[test]
fn versions_updates_and_removal() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.publish(NOW);
    let repo_id = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap().repo_id;
    install(&repo.data, &FileFetch, DAWN, NOW, false).unwrap();
    repo.add(DAWN, "1.1.0");
    repo.publish(NOW + 10);
    let outcome = update_repo(&repo.data, &FileFetch, &repo_id, NOW + 20).unwrap();
    let updates: Vec<_> =
        outcome.updates.iter().map(|u| (u.installed.as_str(), u.available.as_str())).collect();
    assert_eq!(updates, [("1.0.0", "1.1.0")]);
    assert_eq!(install(&repo.data, &FileFetch, DAWN, NOW + 20, false).unwrap().version, "1.1.0");
    assert_eq!(install(&repo.data, &FileFetch, DAWN, NOW + 20, false).unwrap().version, "1.1.0");
    assert!(update_repo(&repo.data, &FileFetch, &repo_id, NOW + 30).unwrap().updates.is_empty());
    let spec = format!("{repo_id}/{DAWN}@1.0.0");
    assert_eq!(install(&repo.data, &FileFetch, &spec, NOW + 20, false).unwrap().version, "1.0.0");
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
    install(&repo.data, &FileFetch, DAWN, NOW, false).unwrap();
    install(&repo.data, &FileFetch, DUSK, NOW, false).unwrap();
    let alone = Repo::new();
    alone.add(DAWN, "1.0.0");
    alone.publish(NOW);
    let path = alone.dir.join("packages").join(DAWN).join("1.0.0.wpak");
    let mut bytes = fs::read(&path).unwrap();
    bytes[40] ^= 1;
    fs::write(&path, bytes).unwrap();
    add_repo(&alone.data, &FileFetch, &alone.url(), NOW).unwrap();
    assert!(matches!(install(&alone.data, &FileFetch, DAWN, NOW, false), Err(Error::Mismatch(_))));
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
    assert!(matches!(install(&repo.data, &FileFetch, DAWN, NOW, false), Err(Error::Mismatch(_))));
}

#[test]
fn author_signatures_are_enforced() {
    let repo = Repo::with(|_, s| s.require_author_signature = true);
    let unsigned = repo.package(DUSK, "1.0.0", None);
    assert!(matches!(add(&repo.dir, &unsigned, None, NOW), Err(Error::Signature(_))));
    let stranger = key();
    let foreign = repo.package("org.example.foreign", "1.0.0", Some(&stranger));
    assert!(matches!(add(&repo.dir, &foreign, None, NOW), Err(Error::Signature(_))));
    assert!(matches!(add(&repo.dir, &foreign, Some("nobody"), NOW), Err(Error::Invalid { .. })));
    assert_eq!(
        add(&repo.dir, &repo.package(DAWN, "1.0.0", Some(&repo.author)), None, NOW)
            .unwrap()
            .author
            .as_deref(),
        Some("studio")
    );
    repo.publish(NOW);
    let repo_id = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap().repo_id;
    install(&repo.data, &FileFetch, DAWN, NOW, false).unwrap();
    repo.add(DAWN, "1.1.0");
    revoke_key(&repo.dir, &repo.author.fingerprint, Reason::Other).unwrap();
    repo.publish(NOW + 10);
    let outcome = update_repo(&repo.data, &FileFetch, &repo_id, NOW + 20).unwrap();
    assert_eq!(outcome.revoked[0].revoked, Some(Reason::Other));
    assert!(matches!(install(&repo.data, &FileFetch, DAWN, NOW + 20, false), Err(Error::Signature(_))));
    assert!(matches!(
        add(&repo.dir, &repo.package(DUSK, "1.0.0", Some(&repo.author)), None, NOW),
        Err(Error::Signature(_))
    ));
}

#[test]
fn stale_repositories_need_confirmation_but_still_revoke() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.add(DUSK, "1.0.0");
    repo.publish(NOW);
    let repo_id = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap().repo_id;
    install(&repo.data, &FileFetch, DAWN, NOW, false).unwrap();
    revoke(&repo.dir, DAWN, None, Reason::Takedown, "").unwrap();
    wprs_format::publish(&repo.dir, &fixture::signer(&repo.online), 1, NOW + 60).unwrap();
    let outcome = update_repo(&repo.data, &FileFetch, &repo_id, NOW + 3 * DAY).unwrap();
    assert!(outcome.repo.stale);
    assert_eq!(outcome.revoked[0].revoked, Some(Reason::Takedown));
    assert!(outcome.repo.warnings.iter().any(|w| w.contains("stale")));
    assert!(matches!(install(&repo.data, &FileFetch, DUSK, NOW + 3 * DAY, false), Err(Error::Stale(_))));
    install(&repo.data, &FileFetch, DUSK, NOW + 3 * DAY, true).unwrap();
}

#[test]
fn ids_specs_and_urls_cannot_escape() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.publish(NOW);
    add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap();
    for spec in
        ["../../etc/passwd", "org.example.dawn@../../x", "../org.example.dawn", "org.example.dawn@1.0.0/../x"]
    {
        assert!(
            matches!(install(&repo.data, &FileFetch, spec, NOW, false), Err(Error::NotFound(_))),
            "{spec}"
        );
    }
    assert!(matches!(update_repo(&repo.data, &FileFetch, "../x", NOW), Err(Error::Invalid { .. })));
    assert!(matches!(remove_repo(&repo.data, ".."), Err(Error::Invalid { .. })));
    assert!(matches!(remove(&repo.data, "../x", None), Err(Error::NotFound(_))));
    for url in [
        "file:///example/repo/?repo_id=../../etc",
        "file:///example/repo/?fingerprint=00",
        "https://example.org/repo/",
    ] {
        assert!(matches!(Anchor::parse(url), Err(Error::Invalid { .. })), "{url}");
    }
    let anchor = Anchor::parse(&format!("https://example.org/r?fingerprint={}", "AB".repeat(32))).unwrap();
    assert_eq!(
        (anchor.base.as_str(), anchor.fingerprint.unwrap()),
        ("https://example.org/r/", "ab".repeat(32))
    );
}

#[test]
fn verify_reports_expiry() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.publish(NOW);
    let report = verify(&FileFetch, repo.dir.to_str().unwrap(), NOW).unwrap();
    assert_eq!((report.entry_version, report.packages), (1, 1));
    assert!(report.warnings.is_empty());
    let soon = verify(&FileFetch, &repo.url(), NOW + 5 * DAY).unwrap();
    assert!(soon.warnings.iter().any(|w| w.contains("72 hours")), "{:?}", soon.warnings);
    assert!(verify(&FileFetch, &repo.url(), NOW + 8 * DAY).unwrap().stale);
    assert!(!repo.data.exists());
    assert!(matches!(publish(&repo.dir, &fixture::signer(&repo.root), 7, NOW), Err(Error::Invalid { .. })));
}
