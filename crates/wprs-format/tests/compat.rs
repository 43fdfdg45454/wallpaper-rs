mod fixture;

use fixture::{DAWN, DUSK, NOW, Repo, files};
use serde::Deserialize;
use std::fs;
use wprs_format::{DETAILED_FORMAT, Entry, Error, FileFetch, Kind, Shard, Time, add_repo, browse};

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
