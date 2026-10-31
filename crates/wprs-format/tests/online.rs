mod fixture;

use fixture::{DAWN, NOW, Repo, key, signer};
use std::collections::BTreeMap;
use wprs_format::{
    Error, FileFetch, add, add_repo, anchors, browse, hide, install, publish, register_author, rotate_author,
    settings, sign_rotation, update_repo,
};

fn online() -> Repo {
    Repo::with(|_, s| {
        s.online_authors = true;
        s.require_author_signature = true;
        s.authors = BTreeMap::new();
    })
}

#[test]
fn a_stolen_online_key_cannot_update_an_installed_package() {
    let repo = online();
    assert!(settings(&repo.dir).unwrap().online_authors);
    let ana = repo.author.clone();
    register_author(&repo.dir, "ana", &ana.public).unwrap();
    let first = add(&repo.dir, &repo.package(DAWN, "1.0.0", Some(&ana)), None, NOW).unwrap();
    assert_eq!(first.author.as_deref(), Some("ana"));
    repo.publish(NOW);
    let info = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap();
    let done = install(&repo.data, &FileFetch, DAWN, NOW, false, false).unwrap();
    assert_eq!(done.signer.as_deref(), Some(ana.fingerprint.as_str()));
    let anchored = anchors(&repo.data).unwrap();
    assert_eq!((anchored.len(), anchored[0].fingerprint.as_str()), (1, ana.fingerprint.as_str()));

    let thief = key();
    register_author(&repo.dir, "ana", &thief.public).unwrap();
    add(&repo.dir, &repo.package(DAWN, "1.1.0", Some(&thief)), None, NOW).unwrap();
    publish(&repo.dir, &signer(&repo.online), 7, NOW + 60).unwrap();
    update_repo(&repo.data, &FileFetch, &info.repo_id, NOW + 120).unwrap();
    let refused = install(&repo.data, &FileFetch, DAWN, NOW + 120, false, false);
    assert!(
        matches!(&refused, Err(Error::Signature(m)) if m.contains("did not hand it over")),
        "{refused:?}"
    );
    assert!(!repo.data.join("packages").join(DAWN).join("1.1.0.wpak").exists());
    assert_eq!(anchors(&repo.data).unwrap()[0].fingerprint, ana.fingerprint);

    let next = key();
    let forged = sign_rotation(&signer(&thief), "ana", &next.public).unwrap();
    let wrong = rotate_author(&repo.dir, "ana", &ana.fingerprint, &next.public, &forged);
    assert!(matches!(wrong, Err(Error::Signature(_))), "{wrong:?}");
    let handed = sign_rotation(&signer(&ana), "ana", &next.public).unwrap();
    rotate_author(&repo.dir, "ana", &ana.fingerprint, &next.public, &handed).unwrap();
    add(&repo.dir, &repo.package(DAWN, "1.2.0", Some(&next)), None, NOW).unwrap();
    publish(&repo.dir, &signer(&repo.online), 7, NOW + 180).unwrap();
    update_repo(&repo.data, &FileFetch, &info.repo_id, NOW + 240).unwrap();
    let rotated = install(&repo.data, &FileFetch, DAWN, NOW + 240, false, false).unwrap();
    assert_eq!(
        (rotated.version.as_str(), rotated.signer.as_deref()),
        ("1.2.0", Some(next.fingerprint.as_str()))
    );
    assert_eq!(anchors(&repo.data).unwrap()[0].fingerprint, next.fingerprint);
}

#[test]
fn root_authors_take_priority_over_online_ones() {
    let repo = Repo::with(|_, s| s.online_authors = true);
    let thief = key();
    let taken = register_author(&repo.dir, "studio", &thief.public);
    assert!(matches!(&taken, Err(Error::Invalid { reason, .. }) if reason.contains("root")), "{taken:?}");
    let refused = add(&repo.dir, &repo.package(DAWN, "1.0.0", Some(&thief)), Some("studio"), NOW);
    assert!(matches!(refused, Err(Error::Signature(_))), "{refused:?}");
    repo.add(DAWN, "1.0.0");
    repo.publish(NOW);
    add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap();
    let done = install(&repo.data, &FileFetch, DAWN, NOW, false, false).unwrap();
    assert_eq!(done.signer.as_deref(), Some(repo.author.fingerprint.as_str()));
    assert!(anchors(&repo.data).unwrap().is_empty());
}

#[test]
fn closed_repositories_refuse_online_authors() {
    let repo = Repo::new();
    assert!(matches!(register_author(&repo.dir, "ana", &key().public), Err(Error::Invalid { .. })));
    repo.add(DAWN, "1.0.0");
    repo.publish(NOW);
    let entry = std::fs::read_to_string(repo.dir.join("entry/1.json")).unwrap();
    assert!(
        !entry.contains("authors")
            && !std::fs::read_to_string(repo.dir.join("root/1.json"))
                .unwrap()
                .contains("\"authors\":\"online\"")
    );
}

#[test]
fn hidden_packages_leave_browsing_but_stay_installable() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.publish(NOW);
    let v1 = std::fs::read_to_string(repo.dir.join("entry/1.json")).unwrap();
    let info = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap();
    install(&repo.data, &FileFetch, DAWN, NOW, false, false).unwrap();
    hide(&repo.dir, DAWN, true).unwrap();
    repo.add(fixture::DUSK, "1.0.0");
    hide(&repo.dir, fixture::DUSK, true).unwrap();
    publish(&repo.dir, &signer(&repo.online), 7, NOW + 60).unwrap();
    let v2: serde_json::Value =
        serde_json::from_slice(&std::fs::read(repo.dir.join("entry/2.json")).unwrap()).unwrap();
    let first = &v2["shards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["sha256"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    let plain =
        first.iter().map(|sha| std::fs::read_to_string(repo.dir.join(format!("index/{sha}.json"))).unwrap());
    assert!(
        plain.clone().all(|shard| !shard.contains("hidden"))
            && plain.clone().any(|shard| shard.contains(DAWN)),
        "{v1}"
    );
    update_repo(&repo.data, &FileFetch, &info.repo_id, NOW + 120).unwrap();
    let shown: Vec<String> = browse(&repo.data, "").unwrap().into_iter().map(|i| i.id).collect();
    assert_eq!(shown, [DAWN]);
    let again = install(&repo.data, &FileFetch, fixture::DUSK, NOW + 120, false, false).unwrap();
    assert_eq!(again.version, "1.0.0");
    hide(&repo.dir, DAWN, false).unwrap();
}
