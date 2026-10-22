use crate::{Error, Result, hex, random};
use hkdf::Hkdf;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use zeroize::Zeroizing;

pub struct SyncKey {
    key: Zeroizing<[u8; 32]>,
    ids: Zeroizing<[u8; 32]>,
}

pub struct Ids(Zeroizing<[u8; 32]>);

fn derive(key: &[u8], info: &[u8]) -> Zeroizing<[u8; 32]> {
    let mut out = Zeroizing::new([0; 32]);
    Hkdf::<Sha256>::new(None, key).expand(info, &mut *out).expect("32 bytes is a valid length");
    out
}

impl SyncKey {
    pub fn generate() -> SyncKey {
        SyncKey { key: Zeroizing::new(random()), ids: Zeroizing::new(random()) }
    }

    pub fn rotated(&self) -> SyncKey {
        SyncKey { key: Zeroizing::new(random()), ids: self.ids.clone() }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<SyncKey> {
        let half =
            |part: &[u8]| -> Zeroizing<[u8; 32]> { Zeroizing::new(part.try_into().unwrap_or([0; 32])) };
        match bytes.len() {
            32 => Ok(SyncKey { key: half(bytes), ids: derive(bytes, b"wprs-sync/ids/v1") }),
            64 => Ok(SyncKey { key: half(&bytes[..32]), ids: half(&bytes[32..]) }),
            _ => Err(Error::Crypto("a sync key has 32 or 64 bytes")),
        }
    }

    pub fn bytes(&self) -> Zeroizing<Vec<u8>> {
        Zeroizing::new([self.key.as_slice(), self.ids.as_slice()].concat())
    }

    pub fn records(&self, epoch: i64) -> Zeroizing<[u8; 32]> {
        derive(&*self.key, &[b"wprs-sync/records/v1".as_slice(), &epoch.to_be_bytes()].concat())
    }

    pub fn ids(&self) -> Ids {
        Ids(self.ids.clone())
    }

    pub fn id(&self) -> String {
        hex(&derive(&*self.key, b"wprs-sync/key-id/v1")[..16])
    }

    pub fn check(&self, epoch: i64) -> Zeroizing<[u8; 32]> {
        derive(&*self.key, &[b"wprs-sync/rotate/v1".as_slice(), &epoch.to_be_bytes()].concat())
    }
}

impl Ids {
    fn raw(&self, parts: &[&[u8]]) -> Zeroizing<[u8; 32]> {
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(&*self.0).expect("HMAC takes any key");
        parts.iter().for_each(|part| mac.update(part));
        Zeroizing::new(mac.finalize().into_bytes().into())
    }

    fn mac(&self, parts: &[&[u8]]) -> String {
        hex(&*self.raw(parts))
    }

    pub fn stored(&self, hash: &[u8]) -> String {
        self.mac(&[b"stored:", hash])
    }

    pub fn file_key(&self, hash: &[u8]) -> Zeroizing<[u8; 32]> {
        self.raw(&[b"wprs-sync/file-key/v1", hash])
    }

    pub fn library(&self, repo_id: &str, package: &str) -> String {
        self.mac(&[repo_id.as_bytes(), &[0], package.as_bytes()])
    }

    pub fn blob(&self, hash: &[u8]) -> String {
        self.mac(&[b"blob:", hash])
    }

    pub fn named(&self, kind: &str, uid: &str) -> String {
        self.mac(&[kind.as_bytes(), &[0], uid.as_bytes()])[..32].to_owned()
    }
}
