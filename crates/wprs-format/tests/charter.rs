use std::collections::BTreeMap;
use std::path::Path;
use wprs_format::{
    Charter, Keypair, Settings, Signer, fingerprint, init, keygen, rotate_root, settings, sign_root,
};

const NOW: i64 = 1_800_000_000;

fn signed(dir: &Path, root: &Keypair) {
    sign_root(dir, &Signer::new(&root.secret, None).unwrap()).unwrap();
}

fn rotated(dir: &Path, root: &Keypair, charter: Charter) -> wprs_format::Result<Settings> {
    let mut next = settings(dir)?;
    charter.apply(&mut next)?;
    rotate_root(dir, &next, NOW)?;
    signed(dir, root);
    settings(dir)
}

fn prints(keys: &[String]) -> Vec<String> {
    keys.iter().map(|k| fingerprint(k).unwrap()).collect()
}

fn authors(id: &str, keys: &[&Keypair]) -> BTreeMap<String, Vec<String>> {
    BTreeMap::from([(id.into(), keys.iter().map(|k| k.public.clone()).collect())])
}

fn made(dir: &Path, root: &Keypair, charter: Charter) {
    let mut first = Settings::default();
    Charter {
        root_keys: vec![root.public.clone()],
        online_keys: vec![keygen(None).unwrap().public],
        ..charter
    }
    .apply(&mut first)
    .unwrap();
    init(dir, &first, NOW).unwrap();
    signed(dir, root);
}

#[test]
fn the_keys_given_for_an_author_replace_the_keys_it_had() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("repo");
    let [root, old, new] = [0; 3].map(|_| keygen(None).unwrap());
    made(&dir, &root, Charter { authors: authors("studio", &[&old]), ..Charter::default() });
    let now =
        rotated(&dir, &root, Charter { authors: authors("studio", &[&new]), ..Charter::default() }).unwrap();
    assert_eq!(
        prints(&now.authors["studio"]),
        prints(std::slice::from_ref(&new.public)),
        "the leaked key stays trusted"
    );
    let same = rotated(&dir, &root, Charter { authors: authors("studio", &[&new]), ..Charter::default() });
    assert_eq!(prints(&same.unwrap().authors["studio"]), prints(std::slice::from_ref(&new.public)));
    let both = Charter { authors: authors("studio", &[&old, &new]), ..Charter::default() };
    assert_eq!(rotated(&dir, &root, both).unwrap().authors["studio"].len(), 2);
}

#[test]
fn mirrors_are_kept_replaced_or_dropped_and_the_root_keeps_its_lifetime() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("repo");
    let root = keygen(None).unwrap();
    let mirror = |url: &str| Some(vec![url.to_string()]);
    made(
        &dir,
        &root,
        Charter { mirrors: mirror("https://a.example"), root_days: Some(90), ..Charter::default() },
    );
    let kept = rotated(&dir, &root, Charter::default()).unwrap();
    assert_eq!((kept.mirrors, kept.root_days), (vec!["https://a.example".to_string()], 90));
    let other = rotated(&dir, &root, Charter { mirrors: mirror("https://b.example"), ..Charter::default() });
    assert_eq!(other.unwrap().mirrors, ["https://b.example"]);
    let none =
        rotated(&dir, &root, Charter { mirrors: Some(vec![]), root_days: Some(400), ..Charter::default() });
    let none = none.unwrap();
    assert!(none.mirrors.is_empty(), "an emptied list of mirrors left {:?}", none.mirrors);
    assert_eq!(none.root_days, 400);
    let charter: Charter = serde_json::from_str(r#"{"mirrors": []}"#).unwrap();
    assert_eq!(charter.mirrors, Some(vec![]));
    assert_eq!(serde_json::from_str::<Charter>("{}").unwrap().mirrors, None);
}

#[test]
fn a_repository_is_made_only_in_a_named_folder() {
    let root = keygen(None).unwrap();
    let mut first = Settings::default();
    let keys = Charter {
        root_keys: vec![root.public],
        online_keys: vec![keygen(None).unwrap().public],
        ..Charter::default()
    };
    keys.apply(&mut first).unwrap();
    let refused = init(Path::new(""), &first, NOW).unwrap_err().to_string();
    assert!(refused.contains("no folder was given for the repository"), "{refused}");
    assert!(!Path::new("root").exists() && !Path::new("catalog.json").exists());
}
