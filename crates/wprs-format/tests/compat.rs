mod fixture;

use fixture::{DAWN, DUSK, NOW, Repo, files};
use serde::Deserialize;
use std::fs;
use wprs_format::{
    DETAILED_FORMAT, Details, Entry, Error, FileFetch, KINDS_FORMAT, Kind, Shard, Time, add_repo, browse,
    repos, update_repo,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OldShard {
    format: String,
    repo_id: String,
    shard: usize,
    packages: Vec<OldListing>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OldListing {
    id: String,
    author: Option<String>,
    versions: Vec<OldRelease>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OldRelease {
    version: String,
    kind: Kind,
    title: String,
    description: String,
    tags: Vec<String>,
    license: String,
    authors: Vec<String>,
    size: u64,
    sha256: String,
    manifest_sha256: String,
    preview: Option<OldPreview>,
    published: Time,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OldPreview {
    sha256: String,
    size: u64,
    ext: String,
}

fn released(repo: &Repo, entry: &Entry) -> Vec<(String, String, String)> {
    let mut found = Vec::new();
    for (index, blob) in entry.shards.iter().enumerate() {
        let bytes = fs::read(repo.dir.join(format!("index/{}.json", blob.sha256))).unwrap();
        let shard: OldShard = serde_json::from_slice(&bytes).unwrap();
        let own = shard.format == "wpak-repo-shard/1.0" && shard.repo_id == entry.repo_id;
        assert!(own && shard.shard == index);
        for OldListing { id, author, versions } in shard.packages {
            assert_eq!(author.as_deref(), Some("studio"));
            for r in versions {
                let preview =
                    r.preview.is_none_or(|p| p.sha256.len() == 64 && p.size > 0 && !p.ext.is_empty());
                let hashes = r.sha256.len() == 64 && r.manifest_sha256.len() == 64;
                let texts = !r.description.is_empty() && !r.tags.is_empty() && !r.authors.is_empty();
                let rest = r.kind == Kind::Image && r.size > 0 && !r.license.is_empty();
                assert!(preview && hashes && texts && rest && r.published.timestamp() == NOW);
                found.push((id.clone(), r.version, r.title));
            }
        }
    }
    found.sort();
    found
}

#[test]
fn released_clients_keep_reading_a_repository_with_version_two_packages() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.add(DUSK, "1.0.0");
    repo.add_v2(DUSK, "2.0.0");
    repo.publish(NOW);
    let entry: Entry = serde_json::from_slice(&fs::read(repo.dir.join("entry/1.json")).unwrap()).unwrap();
    let old = released(&repo, &entry);
    let expected = [(DAWN, "1.0.0", "Sky at dawn"), (DUSK, "1.0.0", "Sky at dusk")];
    assert_eq!(old, expected.map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string())));
    assert_eq!(files(&repo.dir.join("details")), ["1.json", "1.json.minisig"]);
    add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap();
    let dusk = browse(&repo.data, "tormenta").unwrap().remove(0);
    assert_eq!((dusk.id.as_str(), dusk.release.version.as_str()), (DUSK, "2.0.0"));
    assert_eq!(
        (dusk.versions, dusk.release.photosensitive),
        (vec!["1.0.0".into(), "2.0.0".into()], Some(true))
    );
}

#[test]
fn details_are_signed_and_optional() {
    let repo = Repo::new();
    repo.add(DUSK, "1.0.0");
    repo.add_v2(DUSK, "2.0.0");
    repo.publish(NOW);
    let mirror = repo.copy("mirror");
    fs::remove_dir_all(mirror.join("details")).unwrap();
    add_repo(&repo.data, &FileFetch, &repo.url_at(&mirror), NOW).unwrap();
    let found = browse(&repo.data, "").unwrap();
    assert_eq!(
        (found.len(), found[0].release.version.as_str(), found[0].release.content.clone()),
        (1, "1.0.0", None)
    );
    let forged = repo.copy("forged");
    let path = forged.join("details/1.json");
    let text = fs::read_to_string(&path).unwrap().replace("\"version\":1", "\"version\":1 ");
    fs::write(&path, text).unwrap();
    let added = add_repo(&repo.tmp.path().join("other"), &FileFetch, &repo.url_at(&forged), NOW);
    assert!(matches!(added, Err(Error::Signature(_))), "{added:?}");
}

#[test]
fn releases_from_later_publishers_still_parse() {
    let release = |extra: &str| {
        format!(
            r#"{{"version":"1.0.0","kind":"image","title":"Sky","description":"","tags":[],"license":"CC0-1.0",
            "authors":[],"size":1,"sha256":"{0}","manifest_sha256":"{0}","preview":{{"sha256":"{0}","size":1,"ext":"png","width":64}},
            "published":"2027-01-15T08:00:00Z","schema":"wallpaper-rs.wpak/2"{extra}}}"#,
            "b".repeat(64)
        )
    };
    let later = release(
        r#","content":{"rating":"questionable","descriptors":["drugs"],"note":"x"},
        "requires":{"engine":">=9","features":["meshes"],"gpu":"compute"},"listed":true"#,
    );
    let shard = format!(
        r#"{{"format":"{DETAILED_FORMAT}","repo_id":"{}","shard":0,"packages":[{{"id":"{DAWN}","author":null,"versions":[{later}]}}]}}"#,
        "a".repeat(64)
    );
    let shard: Shard = serde_json::from_str(&shard).unwrap();
    shard.check(DETAILED_FORMAT, &"a".repeat(64), 0, 0).unwrap();
    let release = &shard.packages[0].versions[0];
    let content = release.content.as_ref().unwrap();
    assert!(content.mature() && content.descriptors == ["drugs"]);
    assert_eq!(release.requires.as_ref().unwrap().features, ["meshes"]);
}

#[test]
fn clients_skip_releases_of_kinds_they_do_not_know() {
    let release = |version: &str, kind: &str| {
        format!(
            r#"{{"version":"{version}","kind":"{kind}","title":"Sky","description":"","tags":[],"license":"CC0-1.0",
            "authors":[],"size":1,"sha256":"{0}","manifest_sha256":"{0}","preview":null,"published":"2027-01-15T08:00:00Z",
            "schema":"wallpaper-rs.wpak/2","requires":{{"features":["hologram"]}}}}"#,
            "b".repeat(64)
        )
    };
    let listing = |id: &str, versions: &[String]| {
        format!(r#"{{"id":"{id}","author":null,"versions":[{}]}}"#, versions.join(","))
    };
    let packages = [
        listing(DAWN, &[release("1.0.0", "image"), release("2.0.0", "hologram")]),
        listing(DUSK, &[release("1.0.0", "hologram")]),
    ];
    let shard = format!(
        r#"{{"format":"{DETAILED_FORMAT}","repo_id":"{}","shard":0,"packages":[{}]}}"#,
        "a".repeat(64),
        packages.join(",")
    );
    assert!(serde_json::from_str::<Shard>(&shard).is_err());
    let read = Shard::parse(shard.as_bytes()).unwrap();
    read.check(DETAILED_FORMAT, &"a".repeat(64), 0, 0).unwrap();
    let ids: Vec<(&str, usize)> = read.packages.iter().map(|p| (p.id.as_str(), p.versions.len())).collect();
    assert_eq!(ids, [(DAWN, 1)]);
    let empty = shard.replace(&listing(DUSK, &[release("1.0.0", "hologram")]), &listing(DUSK, &[]));
    assert!(Shard::parse(empty.as_bytes()).unwrap().check(DETAILED_FORMAT, &"a".repeat(64), 0, 0).is_err());
}

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
