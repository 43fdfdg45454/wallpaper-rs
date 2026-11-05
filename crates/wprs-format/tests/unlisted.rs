mod fixture;

use fixture::{DAWN, DUSK, NOW, Repo};
use wprs_format::{
    Error, FileFetch, Reason, add_repo, browse, browse_with, feature, install, list, revoke, stock,
    update_repo,
};

fn shards(repo: &Repo, entry: &str, key: &str) -> Vec<String> {
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(repo.dir.join(entry)).unwrap()).unwrap();
    let blobs = value[key].as_array().unwrap().iter().map(|b| b["sha256"].as_str().unwrap().to_owned());
    blobs.map(|sha| std::fs::read_to_string(repo.dir.join(format!("index/{sha}.json"))).unwrap()).collect()
}

#[test]
fn unlisted_releases_leave_browsing_and_search_but_install_by_id() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.add(DUSK, "1.0.0");
    list(&repo.dir, DUSK, None, false).unwrap();
    repo.publish(NOW);
    let plain = shards(&repo, "entry/1.json", "shards");
    assert!(plain.iter().all(|s| !s.contains(DUSK)) && plain.iter().any(|s| s.contains(DAWN)));
    let detailed = shards(&repo, "details/1.json", "shards");
    assert!(detailed.iter().any(|s| s.contains(DUSK) && s.contains("\"listed\":false")));
    let info = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap();
    let shown: Vec<String> = browse(&repo.data, "").unwrap().into_iter().map(|i| i.id).collect();
    assert_eq!(shown, [DAWN]);
    assert!(browse(&repo.data, "dusk").unwrap().is_empty());
    assert_eq!(browse_with(&repo.data, DUSK, true).unwrap()[0].id, DUSK);
    install(&repo.data, &FileFetch, DUSK, NOW, false, false).unwrap();
    let shown: Vec<String> = browse(&repo.data, "").unwrap().into_iter().map(|i| i.id).collect();
    assert_eq!(shown, [DAWN, DUSK]);
    list(&repo.dir, DUSK, Some("1.0.0"), true).unwrap();
    repo.publish(NOW + 60);
    update_repo(&repo.data, &FileFetch, &info.repo_id, NOW + 60).unwrap();
    assert!(shards(&repo, "entry/2.json", "shards").iter().any(|s| s.contains(DUSK)));
}

#[test]
fn listing_names_a_known_package_and_version() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    assert!(matches!(list(&repo.dir, "org.example.none", None, false), Err(Error::NotFound(_))));
    assert!(matches!(list(&repo.dir, DAWN, Some("2.0.0"), false), Err(Error::NotFound(_))));
    assert!(!std::fs::read_to_string(repo.dir.join("catalog.json")).unwrap().contains("listed"));
}

#[test]
fn the_stock_of_a_repository_shows_what_is_listed_featured_and_revoked() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.add(DAWN, "1.1.0");
    repo.add(DUSK, "1.0.0");
    list(&repo.dir, DAWN, Some("1.0.0"), false).unwrap();
    feature(&repo.dir, DUSK, Some("Calm evening"), true).unwrap();
    revoke(&repo.dir, DUSK, None, Reason::Broken, "").unwrap();
    let stocked = stock(&repo.dir).unwrap();
    let ids: Vec<&str> = stocked.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, [DAWN, DUSK]);
    let listed: Vec<bool> = stocked[0].versions.iter().map(|v| v.listed).collect();
    assert_eq!(listed, [false, true]);
    assert!(!stocked[0].featured && stocked[0].versions.iter().all(|v| !v.revoked));
    assert_eq!((stocked[1].featured, stocked[1].note.as_deref()), (true, Some("Calm evening")));
    assert!(stocked[1].versions[0].revoked && stocked[1].versions[0].listed);
}
