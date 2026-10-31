use super::*;

#[derive(Deserialize)]
struct ReleasedShard {
    format: String,
    packages: Vec<ReleasedListing>,
}

#[derive(Deserialize)]
struct ReleasedListing {
    id: String,
    versions: Vec<ReleasedKind>,
}

#[derive(Deserialize)]
struct ReleasedKind {
    kind: ReleasedKinds,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
enum ReleasedKinds {
    Image,
    Video,
    Scene,
}

#[test]
fn released_clients_never_see_kinds_they_cannot_read() {
    let repo = Repo::new();
    repo.add_v2(DUSK, "2.0.0");
    repo.add_page(DAWN, "1.0.0");
    repo.publish(NOW);
    let details: Details =
        serde_json::from_slice(&fs::read(repo.dir.join("details/1.json")).unwrap()).unwrap();
    let read =
        |blob: &wprs_format::Blob| fs::read(repo.dir.join(format!("index/{}.json", blob.sha256))).unwrap();
    let mut seen = Vec::new();
    for blob in &details.shards {
        let shard: ReleasedShard = serde_json::from_slice(&read(blob)).unwrap();
        assert_eq!(shard.format, DETAILED_FORMAT);
        seen.extend(
            shard
                .packages
                .into_iter()
                .flat_map(|l| l.versions.into_iter().map(move |r| (l.id.clone(), r.kind))),
        );
    }
    assert_eq!(seen, [(DUSK.to_string(), ReleasedKinds::Image)]);
    let every = details.kinds.iter().map(|b| Shard::parse(&read(b)).unwrap());
    assert!(every.clone().all(|s| s.format == KINDS_FORMAT));
    assert_eq!(every.map(|s| s.packages.len()).sum::<usize>(), 2);
    add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap();
    let mut found: Vec<_> =
        browse(&repo.data, "").unwrap().into_iter().map(|f| (f.id, f.release.kind)).collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(found, [(DAWN.to_string(), Kind::Web), (DUSK.to_string(), Kind::Image)]);
}

#[test]
fn caches_kept_by_released_clients_still_work_both_ways() {
    let repo = Repo::new();
    repo.add_v2(DUSK, "2.0.0");
    repo.add_page(DAWN, "1.0.0");
    repo.publish(NOW);
    let details: Details =
        serde_json::from_slice(&fs::read(repo.dir.join("details/1.json")).unwrap()).unwrap();
    let id = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap().repo_id;
    let blobs = repo.data.join("repos").join(&id).join("blobs");
    let cached =
        |list: &[wprs_format::Blob]| list.iter().all(|b| blobs.join(format!("{}.json", b.sha256)).exists());
    assert!(cached(&details.shards) && cached(&details.kinds));
    details.kinds.iter().for_each(|b| fs::remove_file(blobs.join(format!("{}.json", b.sha256))).unwrap());
    assert_eq!(repos(&repo.data, NOW).unwrap()[0].repo_id, id);
    let kinds = |found: Vec<wprs_format::Item>| found.into_iter().map(|f| f.release.kind).collect::<Vec<_>>();
    assert_eq!(kinds(browse(&repo.data, "").unwrap()), [Kind::Image]);
    update_repo(&repo.data, &FileFetch, &id, NOW + 10).unwrap();
    assert!(cached(&details.shards) && cached(&details.kinds));
    let mut found = kinds(browse(&repo.data, "").unwrap());
    found.sort_by_key(|k| format!("{k:?}"));
    assert_eq!(found, [Kind::Image, Kind::Web]);
}
