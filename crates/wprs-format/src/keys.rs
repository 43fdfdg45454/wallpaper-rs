use crate::{Error, MANIFEST, Manifest, Result, SIGNATURE, fingerprint, manifest_comment};
use minisign::{KeyPair, PError, PublicKey, SecretKey, SecretKeyBox};
use serde::Serialize;
use std::fs;
use std::path::Path;

impl From<PError> for Error {
    fn from(error: PError) -> Self {
        Error::Key(error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Keypair {
    pub public: String,
    pub secret: String,
    pub fingerprint: String,
}

pub fn keygen(password: Option<String>) -> Result<Keypair> {
    let KeyPair { pk, sk } = match password {
        Some(password) => KeyPair::generate_encrypted_keypair(Some(password))?,
        None => KeyPair::generate_unencrypted_keypair()?,
    };
    let public = pk.to_box()?.into_string();
    let secret = sk.to_box(Some("minisign secret key"))?.into_string();
    Ok(Keypair { fingerprint: fingerprint(&public)?, public, secret })
}

pub struct Signer {
    key: SecretKey,
    pub fingerprint: String,
}

impl Signer {
    pub fn new(secret: &str, password: Option<String>) -> Result<Self> {
        let boxed = SecretKeyBox::from_string(secret)?;
        let key = match password {
            Some(password) => boxed.into_secret_key(Some(password))?,
            None => boxed.into_unencrypted_secret_key()?,
        };
        let fingerprint = fingerprint(&PublicKey::from_secret_key(&key)?.to_base64())?;
        Ok(Signer { key, fingerprint })
    }

    pub fn sign(&self, data: &[u8], comment: &str) -> Result<String> {
        Ok(minisign::sign(None, &self.key, data, Some(comment), None)?.into_string())
    }
}

pub fn sign_source(dir: impl AsRef<Path>, signer: &Signer) -> Result<String> {
    let dir = dir.as_ref();
    let _ = fs::remove_file(dir.join(SIGNATURE));
    crate::fill(dir)?;
    let json = fs::read(dir.join(MANIFEST))?;
    let manifest = Manifest::from_json(&json)?;
    fs::write(dir.join(SIGNATURE), signer.sign(&json, &manifest_comment(&manifest.id, &manifest.version))?)?;
    Ok(signer.fingerprint.clone())
}
