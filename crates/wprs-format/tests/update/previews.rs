use super::*;
use wprs_format::{animated, preview};

#[test]
fn previews_larger_than_a_package_may_carry_are_never_downloaded() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    let catalog = repo.dir.join("catalog.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&catalog).unwrap()).unwrap();
    let huge = |ext: &str| serde_json::json!({ "sha256": "e".repeat(64), "size": 3u64 << 30, "ext": ext });
    value["packages"][0]["versions"][0]["preview"] = huge("webp");
    value["packages"][0]["versions"][0]["animated"] = huge("gif");
    fs::write(&catalog, serde_json::to_vec(&value).unwrap()).unwrap();
    repo.publish(NOW);
    let repo_id = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap().repo_id;
    let (fetch, dest) = (Counting::default(), repo.tmp.path().join("preview"));
    let refused = |e| matches!(e, Error::Limit { actual, .. } if actual == 3 << 30);
    assert!(animated(&repo.data, &fetch, &repo_id, DAWN, "1.0.0", &dest).is_err_and(refused));
    assert!(preview(&repo.data, &fetch, &repo_id, DAWN, "1.0.0", &dest).is_err_and(refused));
    assert_eq!((fetch.count("previews/"), dest.exists()), (0, false));
}
