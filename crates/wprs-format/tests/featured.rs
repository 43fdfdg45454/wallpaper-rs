mod fixture;

use fixture::{DAWN, DUSK, NOW, Repo};
use wprs_format::{Error, Featured, FileFetch, add_repo, browse, feature, featured, update_repo};

#[test]
fn featured_packages_are_signed_into_the_details_and_marked_when_browsing() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.add(DUSK, "1.0.0");
    feature(&repo.dir, DUSK, Some("Calm evening"), true).unwrap();
    repo.publish(NOW);
    let details: serde_json::Value =
        serde_json::from_slice(&std::fs::read(repo.dir.join("details/1.json")).unwrap()).unwrap();
    let sha = details["featured"]["sha256"].as_str().unwrap();
    let bytes = std::fs::read(repo.dir.join(format!("featured/{sha}.json"))).unwrap();
    let repo_id = details["repo_id"].as_str().unwrap();
    assert_eq!(Featured::parse(&bytes, repo_id).unwrap().items[0].note.as_deref(), Some("Calm evening"));
    assert!(Featured::parse(&bytes, "other").is_err());
    let info = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap();
    let marked: Vec<(String, bool)> =
        browse(&repo.data, "").unwrap().into_iter().map(|i| (i.id, i.featured)).collect();
    assert_eq!(marked, [(DAWN.to_string(), false), (DUSK.to_string(), true)]);
    feature(&repo.dir, DUSK, None, false).unwrap();
    repo.publish(NOW + 60);
    update_repo(&repo.data, &FileFetch, &info.repo_id, NOW + 60).unwrap();
    assert!(browse(&repo.data, "").unwrap().iter().all(|i| !i.featured));
}

#[test]
fn featuring_takes_known_packages_and_short_notes() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    assert!(matches!(feature(&repo.dir, "org.example.none", None, true), Err(Error::NotFound(_))));
    assert!(matches!(feature(&repo.dir, DAWN, None, false), Err(Error::NotFound(_))));
    let long = "x".repeat(201);
    for note in [long.as_str(), "<b>", "two\nlines"] {
        assert!(matches!(feature(&repo.dir, DAWN, Some(note), true), Err(Error::Invalid { .. })), "{note:?}");
    }
    assert!(featured(&repo.dir).unwrap().is_empty());
    feature(&repo.dir, DAWN, Some(" "), true).unwrap();
    assert_eq!(featured(&repo.dir).unwrap()[0].note, None);
    let forged = serde_json::json!({"format": "wpak-repo-featured/1.0", "repo_id": "r",
        "items": [{"id": DAWN}, {"id": DAWN}]});
    assert!(Featured::parse(forged.to_string().as_bytes(), "r").is_err());
}
