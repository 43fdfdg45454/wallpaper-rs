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
fn the_key_share_opens_only_from_the_expected_device() {
    let (sender, receiver, other) = (DeviceKey::generate(), DeviceKey::generate(), DeviceKey::generate());
    let key = SyncKey::generate();
    let sealed = seal_key(&sender, &receiver.public(), b"aad", &key).unwrap();
    let opened = open_key(&receiver, &sender.public(), b"aad", &sealed).unwrap();
    assert_eq!(opened.bytes(), key.bytes());
    assert!(open_key(&receiver, &other.public(), b"aad", &sealed).is_err());
    assert!(open_key(&receiver, &sender.public(), b"other", &sealed).is_err());
    assert!(open_key(&other, &sender.public(), b"aad", &sealed).is_err());
    let restored = DeviceKey::from_bytes(&receiver.bytes()).unwrap();
    assert_eq!(restored.public(), receiver.public());
}

#[test]
fn both_devices_show_the_same_code() {
    let (a, b) = (DeviceKey::generate().public(), DeviceKey::generate().public());
    let shown = code(&a, &b);
    assert_eq!(shown, code(&b, &a));
    assert!(shown.len() == 14 && shown.split(' ').all(|g| g.len() == 4 && g.parse::<u32>().is_ok()));
    assert_ne!(shown, code(&a, &DeviceKey::generate().public()));
}

#[test]
fn server_urls_are_checked() {
    assert_eq!(check_url("https://example.org/").unwrap(), "https://example.org");
    assert_eq!(check_url("http://127.0.0.1:8080").unwrap(), "http://127.0.0.1:8080");
    for bad in ["http://example.org", "https://u:p@example.org", "https://example.org/?a=1", "ftp://x"] {
        assert!(check_url(bad).is_err(), "{bad}");
    }
}
