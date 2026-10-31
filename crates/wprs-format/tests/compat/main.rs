#[path = "../fixture/mod.rs"]
mod fixture;

use fixture::{DAWN, DUSK, NOW, Repo, files};
use serde::Deserialize;
use std::fs;
use wprs_format::{
    DETAILED_FORMAT, Details, Entry, Error, FileFetch, KINDS_FORMAT, Kind, Shard, Time, add_repo, browse,
    repos, update_repo,
};

mod kinds;
mod listing;

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
