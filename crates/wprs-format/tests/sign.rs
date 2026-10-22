mod fixture;

use base64::prelude::{BASE64_STANDARD, Engine};
use fixture::{DAWN, Repo, key, pair, signer};
use wprs_format::{
    Error, Limits, Signer, fingerprint, keygen, open, public_key, threshold, verify_blocks, verify_package,
};

const KEY: &str = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
const PREHASHED: &str = "untrusted comment: signature from minisign secret key
RUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=
trusted comment: timestamp:1556193335\tfile:test
y/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==";
const LEGACY: &str = "untrusted comment: signature from minisign secret key
RWQf6LRCGA9i59SLOFxz6NxvASXDJeRtuZykwQepbDEGt87ig1BNpWaVWuNrm73YiIiJbq71Wi+dP9eKL8OC351vwIasSSbXxwA=
trusted comment: timestamp:1555779966\tfile:test
QtKMXWyYcwdpZAlPF7tE2ENJkRd1ujvKjlj1m9RtHTBnZPa5WKU5uWRs5GoP5M/VqE81QFuMKI5k/SfNQUaOAA==";
const COMMENT: &str = "wpak-repo root 1 example";

#[test]
fn golden_vectors() {
    let fp = fingerprint(KEY).unwrap();
    assert_eq!(fp, "faa209af5a6d12495a2e5c690ee94e435084619cc647d0a0a7d82aaeb9a60be6");
    let boxed = format!("untrusted comment: minisign public key E7620F1842B4E81F\n{KEY}\n");
    assert_eq!(public_key(&boxed).unwrap(), KEY);
    let keys = [(fp.as_str(), KEY)];
    let comment = "timestamp:1556193335\tfile:test";
    assert_eq!(threshold(b"test", PREHASHED, comment, &keys, 1).unwrap().len(), 1);
    assert!(matches!(threshold(b"Test", PREHASHED, comment, &keys, 1), Err(Error::Signature(_))));
    let legacy = verify_blocks(b"test", LEGACY, "timestamp:1555779966\tfile:test", &keys).unwrap();
    assert!(legacy.is_empty());
    assert!(matches!(fingerprint("RWQnot-a-key"), Err(Error::Key(_))));
}

#[test]
fn thresholds_count_distinct_keys() {
    let (a, b) = (key(), key());
    let keys = [pair(&a), pair(&b)];
    let (sa, sb) = (signer(&a), signer(&b));
    let sign = |s: &Signer| s.sign(b"data", COMMENT).unwrap();
    let twice = format!("{}{}", sign(&sa), sign(&sa));
    assert!(matches!(threshold(b"data", &twice, COMMENT, &keys, 2), Err(Error::Signature(_))));
    let both = format!("{}{}", sign(&sa), sign(&sb));
    assert_eq!(threshold(b"data", &both, COMMENT, &keys, 2).unwrap().len(), 2);
    assert!(matches!(threshold(b"dat4", &both, COMMENT, &keys, 1), Err(Error::Signature(_))));
    assert!(matches!(
        threshold(b"data", &both, "wpak-repo root 2 example", &keys, 1),
        Err(Error::Signature(_))
    ));
    let mixed = format!("{}{}", sign(&sa), sb.sign(b"data", "wpak-repo root 9 example").unwrap());
    let three_lines: String = both.lines().take(3).map(|l| format!("{l}\n")).collect();
    let broken = both.replacen("\nRU", "\nR!", 1);
    let many = sign(&sa).repeat(17);
    for bad in ["", &mixed, &three_lines, &broken, &many] {
        assert!(matches!(verify_blocks(b"data", bad, COMMENT, &keys), Err(Error::Signature(_))), "{bad}");
    }
}

#[test]
fn copied_key_ids_do_not_help() {
    let (a, b) = (key(), key());
    let mut lines: Vec<String> =
        signer(&b).sign(b"data", COMMENT).unwrap().lines().map(String::from).collect();
    let mut raw = BASE64_STANDARD.decode(&lines[1]).unwrap();
    raw[2..10].copy_from_slice(&BASE64_STANDARD.decode(pair(&a).1).unwrap()[2..10]);
    lines[1] = BASE64_STANDARD.encode(raw);
    let forged = lines.join("\n");
    assert!(verify_blocks(b"data", &forged, COMMENT, &[pair(&a), pair(&b)]).unwrap().is_empty());
}

#[test]
fn encrypted_keys_need_their_password() {
    let pair = keygen(Some("correct horse".into())).unwrap();
    assert!(matches!(Signer::new(&pair.secret, None), Err(Error::Key(_))));
    assert!(matches!(Signer::new(&key().secret, Some("any".into())), Err(Error::Key(_))));
    assert_eq!(
        Signer::new(&pair.secret, Some("correct horse".into())).unwrap().fingerprint,
        pair.fingerprint
    );
}

#[test]
fn package_signatures_cover_the_manifest() {
    let repo = Repo::new();
    let mut package = open(repo.package(DAWN, "1.0.0", Some(&repo.author)), &Limits::default()).unwrap();
    assert_eq!(verify_package(&mut package, &[pair(&repo.author)]).unwrap(), repo.author.fingerprint);
    let other = key();
    assert!(matches!(verify_package(&mut package, &[pair(&other)]), Err(Error::Signature(_))));
    let mut unsigned = open(repo.package("org.example.plain", "1.0.0", None), &Limits::default()).unwrap();
    let result = verify_package(&mut unsigned, &[pair(&repo.author)]);
    assert!(matches!(result, Err(Error::Missing(_))), "{result:?}");
}
