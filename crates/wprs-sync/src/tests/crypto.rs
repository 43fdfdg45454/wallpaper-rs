use crate::*;

fn ctx<'a>(item: &'a str) -> Context<'a> {
    Context { user: 7, epoch: 1, collection: "playlists", item }
}

#[test]
fn records_round_trip_and_bind_their_place() {
    let key = SyncKey::generate().records(1);
    let sealed = seal(&key, &ctx("a1"), br#"{"name":"Sea"}"#).unwrap();
    assert_eq!((sealed.len(), sealed[0]), (41 + 256, 1));
    assert!(!sealed.windows(3).any(|w| w == b"Sea"));
    assert_eq!(open(&key, &ctx("a1"), &sealed).unwrap(), br#"{"name":"Sea"}"#);
    assert!(open(&key, &ctx("a2"), &sealed).is_err());
    let mut flipped = sealed.clone();
    flipped[60] ^= 1;
    assert!(open(&key, &ctx("a1"), &flipped).is_err());
    assert!(open(&SyncKey::generate().records(1), &ctx("a1"), &sealed).is_err());
}

#[test]
fn padding_hides_lengths_in_steps_of_256() {
    for (size, padded) in [(0, 256), (255, 256), (256, 512), (700, 768)] {
        let text = vec![b'x'; size];
        let out = pad(&text);
        assert_eq!(out.len(), padded);
        assert_eq!(unpad(out).unwrap(), text);
    }
    assert!(unpad(vec![0; 256]).is_err());
}

#[test]
fn ids_are_keyed_and_stable() {
    let key = SyncKey::generate();
    let (ids, other) = (key.ids(), SyncKey::generate().ids());
    let id = ids.library("ab", "org.example.dawn");
    assert_eq!(id, ids.library("ab", "org.example.dawn"));
    assert!(id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    assert_ne!(id, other.library("ab", "org.example.dawn"));
    assert_ne!(ids.library("ab", "x"), ids.library("a", "bx"));
    assert_eq!(ids.named("playlist", "X").len(), 32);
    assert_ne!(ids.blob(b"x"), other.blob(b"x"));
    assert_ne!(key.records(1).as_slice(), key.records(2).as_slice());
}

#[test]
fn the_key_share_opens_only_from_the_expected_device_and_place() {
    let (sender, receiver, other) = (DeviceKey::generate(), DeviceKey::generate(), DeviceKey::generate());
    let key = SyncKey::generate();
    let bound = aad(7, "d2", 3, true, &[1; 32]);
    let sealed = seal_key(&sender, &receiver.public(), &bound, &key).unwrap();
    let opened = open_key(&receiver, &sender.public(), &bound, &sealed).unwrap();
    assert_eq!(*opened.bytes(), *key.bytes());
    assert!(open_key(&receiver, &other.public(), &bound, &sealed).is_err());
    for changed in
        [aad(7, "d2", 4, true, &[1; 32]), aad(7, "d2", 3, false, &[1; 32]), aad(8, "d2", 3, true, &[1; 32])]
    {
        assert!(open_key(&receiver, &sender.public(), &changed, &sealed).is_err());
    }
    assert!(open_key(&receiver, &sender.public(), &aad(7, "d2", 3, true, &[2; 32]), &sealed).is_err());
    assert!(open_key(&other, &sender.public(), &bound, &sealed).is_err());
    let restored = DeviceKey::from_bytes(&receiver.bytes()).unwrap();
    assert_eq!(restored.public(), receiver.public());
}

#[test]
fn pairing_codes_change_with_every_key_and_nonce() {
    let (j, h, m) =
        (DeviceKey::generate().public(), DeviceKey::generate().public(), DeviceKey::generate().public());
    let (pair, nj, nh) = ([1u8; 16], [2u8; 32], [3u8; 32]);
    let honest = transcript(&pair, (&j, &nj), (&h, &nh));
    let shown = code(&honest);
    assert_eq!(shown, code(&transcript(&pair, (&j, &nj), (&h, &nh))));
    assert!(shown.len() == 7 && shown.split(' ').all(|g| g.len() == 3 && g.parse::<u32>().is_ok()));
    assert_ne!(honest, transcript(&pair, (&m, &nj), (&h, &nh)));
    assert_ne!(honest, transcript(&pair, (&j, &nj), (&m, &nh)));
    assert_ne!(honest, transcript(&pair, (&j, &nj), (&h, &[4; 32])));
    assert_ne!(honest, transcript(&[9; 16], (&j, &nj), (&h, &nh)));
    assert_eq!(commit(&pair, &nj), commit(&pair, &nj));
    assert_ne!(commit(&pair, &nj), commit(&pair, &nh));
    assert_ne!(commit(&pair, &nj), commit(&[9; 16], &nj));
    let codes: std::collections::BTreeSet<String> =
        (0..64u8).map(|n| code(&transcript(&pair, (&j, &nj), (&h, &[n; 32])))).collect();
    assert!(codes.len() > 60);
}

#[test]
fn device_messages_carry_their_version() {
    let mail = Mail::read(&Mail::body("commit", "ab", b"xyz", 3)).unwrap();
    assert_eq!((mail.step.as_str(), mail.pair.as_str(), mail.epoch), ("commit", "ab", 3));
    assert_eq!(mail.bytes().unwrap(), b"xyz");
    assert!(Mail::read(&[7; 80]).is_err());
    assert!(Mail::read(br#"{"v": 3, "step": "key"}"#).is_err());
}

#[test]
fn rotated_keys_keep_the_item_names_and_change_the_rest() {
    let key = SyncKey::generate();
    let next = key.rotated();
    assert_eq!(next.ids().named("playlist", "x"), key.ids().named("playlist", "x"));
    assert_ne!(next.records(2).as_slice(), key.records(2).as_slice());
    assert_ne!((next.id(), next.check(2).to_vec()), (key.id(), key.check(2).to_vec()));
    let copy = SyncKey::from_bytes(&next.bytes()).unwrap();
    assert_eq!((copy.id(), copy.ids().blob(b"a")), (next.id(), next.ids().blob(b"a")));
    let legacy = SyncKey::from_bytes(&[5; 32]).unwrap();
    let mut derived = [0; 32];
    hkdf::Hkdf::<sha2::Sha256>::new(None, &[5; 32]).expand(b"wprs-sync/ids/v1", &mut derived).unwrap();
    let explicit = SyncKey::from_bytes(&[[5; 32], derived].concat()).unwrap();
    assert_eq!(legacy.ids().named("playlist", "x"), explicit.ids().named("playlist", "x"));
    assert!(SyncKey::from_bytes(&[5; 48]).is_err());
}

#[test]
fn server_urls_are_checked() {
    assert_eq!(check_url("https://example.org/").unwrap(), "https://example.org");
    assert_eq!(check_url("http://127.0.0.1:8080").unwrap(), "http://127.0.0.1:8080");
    for bad in ["http://example.org", "https://u:p@example.org", "https://example.org/?a=1", "ftp://x"] {
        assert!(check_url(bad).is_err(), "{bad}");
    }
}
