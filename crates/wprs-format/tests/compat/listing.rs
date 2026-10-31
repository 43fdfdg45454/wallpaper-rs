use super::*;

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
