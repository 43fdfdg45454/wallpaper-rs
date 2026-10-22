use crate::{Error, Result};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};

pub const SEGMENT: u64 = 65_536;
pub const SEALED: u64 = SEGMENT + 16;

pub fn sealed_size(size: u64) -> u64 {
    size + 16 * size.div_ceil(SEGMENT).max(1)
}

fn parts(key: &[u8; 32], index: u64, last: bool) -> (ChaCha20Poly1305, Nonce) {
    let mut nonce = [0; 12];
    nonce[3..11].copy_from_slice(&index.to_be_bytes());
    nonce[11] = u8::from(last);
    (ChaCha20Poly1305::new_from_slice(key).expect("the key has 32 bytes"), Nonce::from(nonce))
}

pub fn seal_segment(key: &[u8; 32], index: u64, last: bool, plain: &[u8]) -> Result<Vec<u8>> {
    let (cipher, nonce) = parts(key, index, last);
    cipher.encrypt(&nonce, plain).map_err(|_| Error::Crypto("cannot encrypt a file segment"))
}

pub fn open_segment(key: &[u8; 32], index: u64, last: bool, sealed: &[u8]) -> Result<Vec<u8>> {
    let (cipher, nonce) = parts(key, index, last);
    cipher.decrypt(&nonce, sealed).map_err(|_| Error::Crypto("a file segment is damaged or out of place"))
}
