use crate::{Error, Result, hex, random};
use hkdf::Hkdf;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use zeroize::Zeroizing;

pub struct SyncKey(Zeroizing<[u8; 32]>);

pub struct Ids(Zeroizing<[u8; 32]>);

impl SyncKey {
    pub fn generate() -> SyncKey {
        SyncKey(Zeroizing::new(random()))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<SyncKey> {
        let key: [u8; 32] = bytes.try_into().map_err(|_| Error::Crypto("a sync key has 32 bytes"))?;
        Ok(SyncKey(Zeroizing::new(key)))
    }

    pub fn bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn derive(&self, info: &[u8]) -> Zeroizing<[u8; 32]> {
        let mut out = Zeroizing::new([0; 32]);
        Hkdf::<Sha256>::new(None, &*self.0).expand(info, &mut *out).expect("32 bytes is a valid length");
        out
    }

    pub fn records(&self, epoch: i64) -> Zeroizing<[u8; 32]> {
        self.derive(&[b"wprs-sync/records/v1".as_slice(), &epoch.to_be_bytes()].concat())
    }

    pub fn ids(&self) -> Ids {
        Ids(self.derive(b"wprs-sync/ids/v1"))
    }
}

impl Ids {
    fn mac(&self, parts: &[&[u8]]) -> String {
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(&*self.0).expect("HMAC takes any key");
        parts.iter().for_each(|part| mac.update(part));
        hex(&mac.finalize().into_bytes())
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
