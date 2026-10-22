use crate::{Error, Result, random};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};

pub struct Context<'a> {
    pub user: i64,
    pub epoch: i64,
    pub collection: &'a str,
    pub item: &'a str,
}

impl Context<'_> {
    fn aad(&self) -> Vec<u8> {
        let (user, epoch) = (self.user.to_be_bytes(), self.epoch.to_be_bytes());
        [b"wprs-sync/1".as_slice(), &user, &epoch, self.collection.as_bytes(), &[0], self.item.as_bytes()]
            .concat()
    }
}

pub fn pad(text: &[u8]) -> Vec<u8> {
    let mut out = text.to_vec();
    out.push(0x80);
    out.resize(out.len().div_ceil(256) * 256, 0);
    out
}

pub fn unpad(mut text: Vec<u8>) -> Result<Vec<u8>> {
    let end = text.iter().rposition(|b| *b != 0).filter(|at| text[*at] == 0x80);
    text.truncate(end.ok_or(Error::Crypto("the record padding is broken"))?);
    Ok(text)
}

fn cipher(key: &[u8; 32]) -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new_from_slice(key).expect("the key has 32 bytes")
}

pub fn seal(key: &[u8; 32], ctx: &Context, json: &[u8]) -> Result<Vec<u8>> {
    let nonce: [u8; 24] = random();
    let payload = Payload { msg: &pad(json), aad: &ctx.aad() };
    let sealed = cipher(key).encrypt(&XNonce::from(nonce), payload);
    let sealed = sealed.map_err(|_| Error::Crypto("cannot encrypt the record"))?;
    Ok([&[1], nonce.as_slice(), &sealed].concat())
}

pub fn open(key: &[u8; 32], ctx: &Context, payload: &[u8]) -> Result<Vec<u8>> {
    let bad = Error::Crypto("a record cannot be decrypted with this sync key");
    let shaped = payload.len() >= 41 + 256 && payload[0] == 1 && (payload.len() - 41).is_multiple_of(256);
    if !shaped {
        return Err(bad);
    }
    let nonce = XNonce::try_from(&payload[1..25]).map_err(|_| Error::Crypto("bad nonce"))?;
    let text = cipher(key).decrypt(&nonce, Payload { msg: &payload[25..], aad: &ctx.aad() });
    unpad(text.map_err(|_| bad)?)
}
