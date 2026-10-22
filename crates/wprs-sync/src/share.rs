use crate::{Error, Result, SyncKey};
use argon2::{Algorithm, Argon2, Params, Version};
use hpke::aead::ChaCha20Poly1305;
use hpke::kdf::HkdfSha256;
use hpke::kem::X25519HkdfSha256 as Kem;
use hpke::{Deserializable, Kem as _, OpModeR, OpModeS, Serializable};
use zeroize::Zeroizing;

const INFO: &[u8] = b"wprs key-share v1";
const SALT: &[u8] = b"wprs-sync/verify/v1";

type Secret = <Kem as hpke::Kem>::PrivateKey;
type Public = <Kem as hpke::Kem>::PublicKey;
type Encapped = <Kem as hpke::Kem>::EncappedKey;

pub struct DeviceKey(Secret);

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
        key.bytes(),
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

pub fn code(a: &[u8], b: &[u8]) -> String {
    let (low, high) = if a <= b { (a, b) } else { (b, a) };
    let params = Params::new(4096, 2, 1, Some(8)).expect("valid parameters");
    let mut out = [0; 8];
    let hashed = Argon2::new(Algorithm::Argon2id, Version::V0x13, params).hash_password_into(
        &[low, high].concat(),
        SALT,
        &mut out,
    );
    hashed.expect("valid input lengths");
    let n = u64::from_be_bytes(out) % 1_000_000_000_000;
    format!("{:04} {:04} {:04}", n / 100_000_000, n / 10_000 % 10_000, n % 10_000)
}
