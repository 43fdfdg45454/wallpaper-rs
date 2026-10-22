use crate::{Error, Result, SyncKey};
use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use hpke::aead::ChaCha20Poly1305;
use hpke::kdf::HkdfSha256;
use hpke::kem::X25519HkdfSha256 as Kem;
use hpke::{Deserializable, Kem as _, OpModeR, OpModeS, Serializable};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

const INFO: &[u8] = b"wprs key-share v2";
pub const VERSION: u32 = 2;

type Secret = <Kem as hpke::Kem>::PrivateKey;
type Public = <Kem as hpke::Kem>::PublicKey;
type Encapped = <Kem as hpke::Kem>::EncappedKey;

pub struct DeviceKey(Secret);

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Mail {
    pub v: u32,
    pub step: String,
    #[serde(default)]
    pub pair: String,
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    pub epoch: i64,
}

impl DeviceKey {
    pub fn generate() -> DeviceKey {
        DeviceKey(Kem::gen_keypair().0)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<DeviceKey> {
        Secret::from_bytes(bytes).map(DeviceKey).map_err(|_| Error::Crypto("the device key is broken"))
    }

    pub fn bytes(&self) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(self.0.to_bytes().to_vec())
    }

    pub fn public(&self) -> Vec<u8> {
        Kem::sk_to_pk(&self.0).to_bytes().to_vec()
    }
}

fn public(bytes: &[u8]) -> Result<Public> {
    Public::from_bytes(bytes).map_err(|_| Error::Crypto("the public key of the device is broken"))
}

pub fn seal_key(from: &DeviceKey, to: &[u8], aad: &[u8], key: &SyncKey) -> Result<Vec<u8>> {
    let mode = OpModeS::Auth((from.0.clone(), Kem::sk_to_pk(&from.0)));
    let sealed = hpke::single_shot_seal::<ChaCha20Poly1305, HkdfSha256, Kem>(
        &mode,
        &public(to)?,
        INFO,
        &key.bytes(),
        aad,
    );
    let (enc, sealed) = sealed.map_err(|_| Error::Crypto("cannot seal the sync key"))?;
    Ok([enc.to_bytes().as_slice(), &sealed].concat())
}

pub fn open_key(to: &DeviceKey, from: &[u8], aad: &[u8], sealed: &[u8]) -> Result<SyncKey> {
    let bad = Error::Crypto("the key share does not come from that device or was changed");
    let (enc, rest) = sealed.split_at_checked(32).ok_or(Error::Crypto("the key share is too short"))?;
    let enc = Encapped::from_bytes(enc).map_err(|_| Error::Crypto("the key share is broken"))?;
    let mode = OpModeR::Auth(public(from)?);
    let opened =
        hpke::single_shot_open::<ChaCha20Poly1305, HkdfSha256, Kem>(&mode, &to.0, &enc, INFO, rest, aad);
    SyncKey::from_bytes(&Zeroizing::new(opened.map_err(|_| bad)?))
}

fn hash(parts: &[&[u8]]) -> [u8; 32] {
    parts.iter().fold(Sha256::new(), |h, part| h.chain_update(part)).finalize().into()
}

pub fn commit(pair: &[u8], nonce: &[u8]) -> [u8; 32] {
    hash(&[b"wprs-sync/commit/v2", pair, nonce])
}

pub fn transcript(pair: &[u8], joining: (&[u8], &[u8]), holding: (&[u8], &[u8])) -> [u8; 32] {
    hash(&[b"wprs-sync/pair/v2", pair, joining.0, holding.0, joining.1, holding.1])
}

pub fn code(transcript: &[u8; 32]) -> String {
    let n = u64::from_be_bytes(transcript[..8].try_into().unwrap_or_default()) % 1_000_000;
    format!("{:03} {:03}", n / 1000, n % 1000)
}

pub fn aad(user: i64, device: &str, epoch: i64, encrypted: bool, bound: &[u8]) -> Vec<u8> {
    let (user, epoch) = (user.to_be_bytes(), epoch.to_be_bytes());
    [&user, device.as_bytes(), &epoch, &[u8::from(encrypted)], bound].concat()
}

impl Mail {
    pub fn body(step: &str, pair: &str, data: &[u8], epoch: i64) -> Vec<u8> {
        let mail = Mail { v: VERSION, step: step.into(), pair: pair.into(), data: B64.encode(data), epoch };
        serde_json::to_vec(&mail).unwrap_or_default()
    }

    pub fn read(body: &[u8]) -> Result<Mail> {
        let old =
            || Error::Invalid("the other device runs another version of wallpaper-rs; update both".into());
        let mail: Mail = serde_json::from_slice(body).map_err(|_| old())?;
        if mail.v != VERSION { Err(old()) } else { Ok(mail) }
    }

    pub fn bytes(&self) -> Result<Vec<u8>> {
        B64.decode(&self.data).map_err(|_| Error::Invalid("a device message is broken".into()))
    }
}
