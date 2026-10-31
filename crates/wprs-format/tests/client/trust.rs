use super::*;

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
    install(&repo.data, &FileFetch, DAWN, NOW, false, false).unwrap();
    repo.add(DAWN, "1.1.0");
    revoke_key(&repo.dir, &repo.author.fingerprint, Reason::Other).unwrap();
    repo.publish(NOW + 10);
    let outcome = update_repo(&repo.data, &FileFetch, &repo_id, NOW + 20).unwrap();
    assert_eq!(outcome.revoked[0].revoked, Some(Reason::Other));
    assert!(matches!(
        install(&repo.data, &FileFetch, DAWN, NOW + 20, false, false),
        Err(Error::Signature(_))
    ));
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
    install(&repo.data, &FileFetch, DAWN, NOW, false, false).unwrap();
    revoke(&repo.dir, DAWN, None, Reason::Takedown, "").unwrap();
    wprs_format::publish(&repo.dir, &fixture::signer(&repo.online), 1, NOW + 60).unwrap();
    let outcome = update_repo(&repo.data, &FileFetch, &repo_id, NOW + 3 * DAY).unwrap();
    assert!(outcome.repo.stale);
    assert_eq!(outcome.revoked[0].revoked, Some(Reason::Takedown));
    assert!(outcome.repo.warnings.iter().any(|w| w.contains("stale")));
    assert!(matches!(
        install(&repo.data, &FileFetch, DUSK, NOW + 3 * DAY, false, false),
        Err(Error::Stale(_))
    ));
    install(&repo.data, &FileFetch, DUSK, NOW + 3 * DAY, true, false).unwrap();
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
            matches!(install(&repo.data, &FileFetch, spec, NOW, false, false), Err(Error::NotFound(_))),
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
