use crate::*;

const LIGHT: Cost = Cost { memory_kib: 64, passes: 1 };
const WORDS: &str = "copper lantern drifts over quiet harbor";

#[test]
fn the_escrow_opens_only_with_its_passphrase_and_account() {
    assert!(Lock::new("short phrase", LIGHT).is_err());
    let (key, lock) = (SyncKey::generate(), Lock::new(WORDS, LIGHT).unwrap());
    let escrow = lock.seal(&key, 7, 3).unwrap();
    assert_eq!((escrow.len(), escrow_epoch(&escrow)), (137, Some(3)));
    assert!(!escrow.windows(8).any(|w| key.bytes().windows(8).any(|k| k == w)));
    let (opened, epoch, again) = unseal(&escrow, WORDS, 7).unwrap();
    assert_eq!((opened.bytes().to_vec(), epoch), (key.bytes().to_vec(), 3));
    assert!(again.fits(&escrow) && lock.fits(&escrow));
    assert!(unseal(&escrow, "copper lantern drifts over quiet harbour", 7).is_err());
    assert!(unseal(&escrow, WORDS, 8).is_err());
    let mut moved = escrow.clone();
    moved[32] ^= 1;
    assert!(unseal(&moved, WORDS, 7).is_err());
    let cached = Lock::from_bytes(&lock.bytes()).unwrap();
    let rotated = key.rotated();
    let next = cached.seal(&rotated, 7, 4).unwrap();
    assert_eq!(*unseal(&next, WORDS, 7).unwrap().0.bytes(), *rotated.bytes());
    assert!(!Lock::new(WORDS, LIGHT).unwrap().fits(&next));
    let mut greedy = next.clone();
    greedy[1..5].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(unseal(&greedy, WORDS, 7).err().unwrap().to_string().contains("unusable"));
    assert_eq!(STRONG, Cost { memory_kib: 262_144, passes: 3 });
}

#[test]
fn file_segments_open_only_in_their_place() {
    let ids = SyncKey::generate().ids();
    let key = ids.file_key(b"hash");
    assert_eq!(*key, *ids.file_key(b"hash"));
    assert_ne!(*key, *ids.file_key(b"other"));
    assert_ne!(ids.stored(b"hash"), SyncKey::generate().ids().stored(b"hash"));
    assert_eq!(ids.stored(b"hash").len(), 64);
    let sealed = seal_segment(&key, 4, false, b"pixels").unwrap();
    assert_eq!(sealed.len(), 6 + 16);
    assert_eq!(seal_segment(&key, 4, false, b"pixels").unwrap(), sealed);
    assert_eq!(open_segment(&key, 4, false, &sealed).unwrap(), b"pixels");
    assert!(open_segment(&key, 5, false, &sealed).is_err());
    assert!(open_segment(&key, 4, true, &sealed).is_err());
    assert!(open_segment(&ids.file_key(b"other"), 4, false, &sealed).is_err());
    assert_eq!([sealed_size(0), sealed_size(SEGMENT), sealed_size(SEGMENT + 1)], [16, SEALED, SEALED + 17]);
}
