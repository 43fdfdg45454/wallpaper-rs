#[path = "../fixture/mod.rs"]
mod fixture;

use fixture::{Counting, DAWN, DAY, DUSK, NOW, Repo, file_url, files, key, signer};
use std::fs;
use std::path::Path;
use wprs_format::{
    Error, FileFetch, add_repo, comment, publish, repos, rotate_root, settings, sign_root, status,
    update_repo,
};

mod anchors;
mod shards;

fn resign(repo: &Repo, path: &str, edit: impl FnOnce(&mut serde_json::Value)) {
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(repo.dir.join(path)).unwrap()).unwrap();
    edit(&mut value);
    let bytes = serde_json::to_vec(&value).unwrap();
    let version = value["version"].as_u64().unwrap();
    let sig =
        signer(&repo.online).sign(&bytes, &comment("entry", version, &status(&repo.dir).unwrap().repo_id));
    fs::write(repo.dir.join(path), &bytes).unwrap();
    fs::write(repo.dir.join(format!("{path}.minisig")), sig.unwrap()).unwrap();
}

fn state(data: &Path) -> Vec<u8> {
    let dir = fs::read_dir(data.join("repos")).unwrap().next().unwrap().unwrap().path();
    fs::read(dir.join("state.json")).unwrap()
}
