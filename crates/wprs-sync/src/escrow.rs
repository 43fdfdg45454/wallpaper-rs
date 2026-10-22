use crate::{Error, Result, SyncKey, random};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

pub const STRONG: Cost = Cost { memory_kib: 262_144, passes: 3 };
pub const MIN_PASSPHRASE: usize = 20;
const HEAD: usize = 33;
const SIZE: usize = HEAD + 24 + 64 + 16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cost {
    pub memory_kib: u32,
    pub passes: u32,
}

pub struct Lock {
    cost: Cost,
    salt: [u8; 16],
    key: Zeroizing<[u8; 32]>,
}

fn derive(passphrase: &str, salt: &[u8; 16], cost: Cost) -> Result<Zeroizing<[u8; 32]>> {
    let odd = Error::Crypto("the recovery escrow asks for an unusable key derivation");
    if !(8..=1 << 20).contains(&cost.memory_kib) || !(1..=10).contains(&cost.passes) {
        return Err(odd);
    }
    let params = Params::new(cost.memory_kib, cost.passes, 1, Some(32)).map_err(|_| odd)?;
    let mut key = Zeroizing::new([0; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), salt, &mut *key)
        .map_err(|_| Error::Crypto("the passphrase cannot be turned into a key"))?;
    Ok(key)
}

fn aad(user: i64, head: &[u8]) -> Vec<u8> {
    [b"wprs-sync/escrow/v1".as_slice(), &user.to_be_bytes(), head].concat()
}

fn cipher(key: &[u8; 32]) -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new_from_slice(key).expect("the key has 32 bytes")
}

impl Lock {
    pub fn new(passphrase: &str, cost: Cost) -> Result<Lock> {
        if passphrase.trim().chars().count() < MIN_PASSPHRASE {
            return Err(Error::Invalid(format!(
                "a recovery passphrase has at least {MIN_PASSPHRASE} characters; several words are best"
            )));
        }
        let salt = random();
        Ok(Lock { cost, salt, key: derive(passphrase, &salt, cost)? })
    }

    pub fn bytes(&self) -> Zeroizing<Vec<u8>> {
        let cost = [self.cost.memory_kib.to_be_bytes(), self.cost.passes.to_be_bytes()].concat();
        Zeroizing::new([cost.as_slice(), &self.salt, &*self.key].concat())
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Lock> {
        let num = |at: usize| Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
        let cost = Cost { memory_kib: num(0)?, passes: num(4)? };
        let salt = bytes.get(8..24)?.try_into().ok()?;
        let key = Zeroizing::new(bytes.get(24..56)?.try_into().ok()?);
        Some(Lock { cost, salt, key })
    }

    pub fn fits(&self, escrow: &[u8]) -> bool {
        escrow.len() == SIZE && escrow[9..25] == self.salt
    }

    pub fn seal(&self, key: &SyncKey, user: i64, epoch: i64) -> Result<Vec<u8>> {
        let (m, t) = (self.cost.memory_kib.to_be_bytes(), self.cost.passes.to_be_bytes());
        let head = [[1].as_slice(), &m, &t, &self.salt, &epoch.to_be_bytes()].concat();
        let nonce: [u8; 24] = random();
        let payload = Payload { msg: &key.bytes(), aad: &aad(user, &head) };
        let sealed = cipher(&self.key).encrypt(&XNonce::from(nonce), payload);
        let sealed = sealed.map_err(|_| Error::Crypto("cannot seal the recovery escrow"))?;
        Ok([head.as_slice(), &nonce, &sealed].concat())
    }
}

pub fn escrow_epoch(escrow: &[u8]) -> Option<i64> {
    let bytes = escrow.get(25..HEAD).filter(|_| escrow.len() == SIZE && escrow[0] == 1)?;
    Some(i64::from_be_bytes(bytes.try_into().ok()?))
}

pub fn unseal(escrow: &[u8], passphrase: &str, user: i64) -> Result<(SyncKey, i64, Lock)> {
    let epoch = escrow_epoch(escrow).ok_or(Error::Crypto("the recovery escrow is broken"))?;
    let lock = Lock::from_bytes(&[&escrow[1..25], [0; 32].as_slice()].concat()).expect("the head fits");
    let lock = Lock { key: derive(passphrase, &lock.salt, lock.cost)?, ..lock };
    let (head, rest) = escrow.split_at(HEAD);
    let nonce = XNonce::try_from(&rest[..24]).map_err(|_| Error::Crypto("bad nonce"))?;
    let opened = cipher(&lock.key).decrypt(&nonce, Payload { msg: &rest[24..], aad: &aad(user, head) });
    let opened = opened.map_err(|_| Error::Crypto("the passphrase does not open the recovery escrow"))?;
    Ok((SyncKey::from_bytes(&Zeroizing::new(opened))?, epoch, lock))
}
