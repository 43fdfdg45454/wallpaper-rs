mod fixture;

use fixture::{DAWN, DUSK, NOW, Repo, files, key, signer};
use std::collections::BTreeMap;
use std::fs;
use wprs_format::{
    Entry, Error, Kind, Listing, Preview, REVOKED_FORMAT, Reason, Release, Revocation, Revoked, Root,
    SHARD_FORMAT, Settings, Shard, Time, add, init, public_key, publish, settings, shard_of, sign_root,
};

#[test]
fn init_validates_settings_and_signing() {
    let (root, online) = (key(), key());
    let base = Settings {
        name: "Example".into(),
        root_keys: vec![root.public.clone()],
        online_keys: vec![online.public.clone()],
        ..Settings::default()
    };
    let author = |id: &str| BTreeMap::from([(id.to_string(), vec![key().public])]);
    let cases = [
        Settings { root_threshold: 0, ..base.clone() },
        Settings { root_threshold: 2, ..base.clone() },
        Settings { online_keys: vec![root.public.clone()], ..base.clone() },
        Settings { online_keys: vec!["RWQnot-a-key".into()], ..base.clone() },
        Settings { root_days: 10, ..base.clone() },
        Settings { entry_max_days: 91, ..base.clone() },
        Settings { shard_bits: 9, ..base.clone() },
        Settings { mirrors: vec!["http://example.org/".into()], ..base.clone() },
        Settings { authors: author("Not Valid"), ..base.clone() },
    ];
    let tmp = tempfile::tempdir().unwrap();
    for (n, settings) in cases.iter().enumerate() {
        assert!(init(&tmp.path().join(n.to_string()), settings, NOW).is_err(), "{settings:?}");
        assert!(!tmp.path().join(n.to_string()).join("root/1.json").exists());
    }
    let dir = tmp.path().join("repo");
    let created = init(&dir, &base, NOW).unwrap();
    assert_eq!((created.version, created.threshold, created.complete), (1, 1, false));
    assert!(matches!(init(&dir, &base, NOW), Err(Error::Invalid { .. })));
    assert!(matches!(publish(&dir, &signer(&online), 7, NOW), Err(Error::Invalid { .. })));
    assert!(matches!(sign_root(&dir, &signer(&online)), Err(Error::Invalid { .. })));
    let signed = sign_root(&dir, &signer(&root)).unwrap();
    assert!(signed.complete);
    assert_eq!(signed.signers, [root.fingerprint.as_str()]);
    assert_eq!(signed.query, format!("fingerprint={}&repo_id={}", root.fingerprint, signed.repo_id));
    let stored = settings(&dir).unwrap();
    assert_eq!(stored.root_keys, [public_key(&root.public).unwrap()]);
    assert_eq!(
        Settings { root_keys: base.root_keys.clone(), online_keys: base.online_keys.clone(), ..stored },
        base
    );
}

#[test]
fn published_layout_and_strict_metadata() {
    let repo = Repo::new();
    let wpak = repo.package(DAWN, "1.0.0", Some(&repo.author));
    let added = add(&repo.dir, &wpak, Some("studio"), NOW).unwrap();
    assert_eq!(add(&repo.dir, &wpak, None, NOW).unwrap(), added);
    let again = repo.package(DAWN, "1.0.0", Some(&repo.author));
    assert!(
        matches!(add(&repo.dir, &again, None, NOW), Err(Error::Invalid { field, .. }) if field == "version")
    );
    let preview = added.release.preview.clone().unwrap();
    assert_eq!((added.release.kind, preview.ext.as_str(), preview.size), (Kind::Image, "webp", 12));
    repo.publish(NOW);
    assert_eq!(
        files(&repo.dir),
        ["catalog.json", "entry", "index", "packages", "previews", "revoked", "root"]
    );
    assert_eq!(files(&repo.dir.join("entry")), ["1.json", "1.json.minisig", "latest"]);
    assert_eq!(files(&repo.dir.join("root")), ["1.json", "1.json.minisig"]);
    assert_eq!(files(&repo.dir.join("previews")), [format!("{}.webp", preview.sha256)]);
    assert_eq!(files(&repo.dir.join("packages").join(DAWN)), ["1.0.0.wpak"]);
    assert_eq!(fs::read_to_string(repo.dir.join("entry/latest")).unwrap(), "1\n");
    let entry_json = fs::read_to_string(repo.dir.join("entry/1.json")).unwrap();
    let entry: Entry = serde_json::from_str(&entry_json).unwrap();
    assert_eq!((entry.version, entry.shards.len(), entry.shard_bits), (1, 16, 4));
    assert_eq!(files(&repo.dir.join("index")).len(), 16);
    assert!(serde_json::from_str::<Entry>(&entry_json.replacen('{', "{\"extra\":1,", 1)).is_err());
    let mut root: Root = serde_json::from_slice(&fs::read(repo.dir.join("root/1.json")).unwrap()).unwrap();
    let id = entry.repo_id.clone();
    root.check(1, &id).unwrap();
    root.version = 2;
    assert!(root.check(2, &id).is_err());
    root.genesis = Some("e".repeat(64));
    assert!(root.check(2, &id).is_err());
    root.genesis = Some(id.clone());
    root.check(2, &id).unwrap();
    let (fp, key) = root.keys.pop_first().unwrap();
    root.keys.insert("0".repeat(64), key);
    assert_ne!(fp, "0".repeat(64));
    assert!(root.check(2, &id).is_err());
}

fn release(version: &str) -> Release {
    Release {
        version: version.into(),
        kind: Kind::Image,
        title: "Sky".into(),
        description: String::new(),
        tags: Vec::new(),
        license: "CC0-1.0".into(),
        authors: Vec::new(),
        size: 1,
        sha256: "b".repeat(64),
        manifest_sha256: "c".repeat(64),
        preview: None,
        published: Time::from_timestamp(NOW, 0).unwrap(),
    }
}

fn listing(id: &str, versions: &[&str]) -> Listing {
    Listing { id: id.into(), author: None, versions: versions.iter().map(|v| release(v)).collect() }
}

#[test]
fn shards_reject_misplacement_and_path_tricks() {
    let repo_id = "a".repeat(64);
    let index = shard_of(DAWN, 4);
    let shard =
        |packages| Shard { format: SHARD_FORMAT.into(), repo_id: repo_id.clone(), shard: index, packages };
    shard(vec![listing(DAWN, &["0.3.0-beta.1", "1.0.0", "1.2.0"])]).check(&repo_id, index, 4).unwrap();
    let elsewhere = (0..).map(|n| format!("org.example.n{n}")).find(|id| shard_of(id, 4) != index).unwrap();
    let hash = Release { sha256: "../../x".into(), ..release("1.0.0") };
    let preview = Preview { sha256: "d".repeat(64), size: 1, ext: "../png".into() };
    let ext = Release { preview: Some(preview), ..release("1.0.0") };
    let bad = [
        vec![listing(DAWN, &["1.2.0", "1.0.0"])],
        vec![listing(DAWN, &["1.0.0", "1.0.0"])],
        vec![listing(DAWN, &[])],
        vec![listing(DAWN, &["1.0.0/../../x"])],
        vec![listing(DAWN, &["../1.0.0"])],
        vec![listing(DAWN, &["v1.0.0"])],
        vec![listing(DAWN, &["1.0.0"]), listing(DAWN, &["2.0.0"])],
        vec![listing("../../etc", &["1.0.0"])],
        vec![listing("org.example/../x", &["1.0.0"])],
        vec![listing(&elsewhere, &["1.0.0"])],
        vec![Listing { versions: vec![hash], ..listing(DAWN, &[]) }],
        vec![Listing { versions: vec![ext], ..listing(DAWN, &[]) }],
    ];
    for packages in bad {
        let shard = shard(packages);
        assert!(matches!(shard.check(&repo_id, index, 4), Err(Error::Invalid { .. })), "{shard:?}");
    }
    assert!(shard(Vec::new()).check(&"d".repeat(64), index, 4).is_err());
    assert!(shard(Vec::new()).check(&repo_id, (index + 1) % 16, 4).is_err());
    assert_eq!(
        (shard_of("01J9Z0X9Z6Q7X4F3K5E1V0R1AB", 4), shard_of("01J9Z0X9Z6Q7X4F3K5E1V0R1AB", 8)),
        (9, 156)
    );
    assert_eq!(shard_of(DUSK, 0), 0);
}

#[test]
fn revocations_match_versions_and_keys() {
    let repo_id = "a".repeat(64);
    let all = Revocation {
        id: DAWN.into(),
        versions: vec!["*".into()],
        reason: Reason::Broken,
        message: String::new(),
    };
    let one = Revocation {
        id: DUSK.into(),
        versions: vec!["1.0.0".into()],
        reason: Reason::License,
        message: String::new(),
    };
    let key = wprs_format::KeyRevocation { fingerprint: "f".repeat(64), reason: Reason::Malware };
    let revoked = Revoked {
        format: REVOKED_FORMAT.into(),
        repo_id: repo_id.clone(),
        packages: vec![all, one],
        keys: vec![key],
    };
    revoked.check(&repo_id).unwrap();
    assert!(revoked.check(&"d".repeat(64)).is_err());
    assert_eq!(revoked.version(DAWN, "9.9.9"), Some(Reason::Broken));
    assert_eq!(
        (revoked.version(DUSK, "1.0.0"), revoked.version(DUSK, "1.0.1")),
        (Some(Reason::License), None)
    );
    assert_eq!((revoked.key(&"f".repeat(64)), revoked.key(&"e".repeat(64))), (Some(Reason::Malware), None));
    let json = serde_json::to_string(&revoked).unwrap().replace("broken", "boring");
    assert!(serde_json::from_str::<Revoked>(&json).is_err());
}
