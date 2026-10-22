use crate::{Error, MANIFEST, Package, Result, SIGNATURE, ensure, sha256};
use base64::prelude::{BASE64_STANDARD, Engine};
use minisign_verify::{PublicKey, Signature};
use std::collections::BTreeSet;
use std::io::{Read, Seek};

const MAX_BLOCKS: usize = 16;

pub fn public_key(text: &str) -> Result<String> {
    let line = text.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or_default();
    PublicKey::from_base64(line).map_err(|e| Error::Key(e.to_string()))?;
    Ok(line.into())
}

pub fn fingerprint(key: &str) -> Result<String> {
    let raw = BASE64_STANDARD.decode(public_key(key)?).map_err(|e| Error::Key(e.to_string()))?;
    sha256(&raw[10..])
}

pub fn manifest_comment(id: &str, version: &str) -> String {
    format!("wpak manifest {id} {version}")
}

pub fn verify_blocks(
    data: &[u8],
    sigs: &str,
    comment: &str,
    keys: &[(&str, &str)],
) -> Result<BTreeSet<String>> {
    let lines: Vec<&str> = sigs.lines().collect();
    let blocks = lines.len() / 4;
    let shaped = lines.len().is_multiple_of(4) && (1..=MAX_BLOCKS).contains(&blocks);
    ensure(shaped, || Error::Signature("expected 1 to 16 minisign blocks".into()))?;
    let mut verified = BTreeSet::new();
    for block in lines.chunks(4) {
        let sig = Signature::decode(&block.join("\n")).map_err(|e| Error::Signature(e.to_string()))?;
        let expected = sig.trusted_comment() == comment;
        ensure(expected, || Error::Signature(format!("trusted comment is not {comment:?}")))?;
        let valid = keys.iter().filter(|(_, key)| {
            PublicKey::from_base64(key).is_ok_and(|key| key.verify(data, &sig, false).is_ok())
        });
        verified.extend(valid.map(|(fp, _)| fp.to_string()));
    }
    Ok(verified)
}

pub fn threshold(
    data: &[u8],
    sigs: &str,
    comment: &str,
    keys: &[(&str, &str)],
    threshold: usize,
) -> Result<BTreeSet<String>> {
    let verified = verify_blocks(data, sigs, comment, keys)?;
    let count = verified.len();
    ensure(count >= threshold, || Error::Signature(format!("{count} of {threshold} required keys signed")))?;
    Ok(verified)
}

pub fn verify_package<R: Read + Seek>(package: &mut Package<R>, keys: &[(&str, &str)]) -> Result<String> {
    let sig = String::from_utf8(package.read_file(SIGNATURE)?)
        .map_err(|_| Error::Signature(format!("{SIGNATURE} is not UTF-8")))?;
    let comment = manifest_comment(&package.manifest.id, &package.manifest.version);
    let verified = threshold(&package.read_file(MANIFEST)?, &sig, &comment, keys, 1)?;
    Ok(verified.into_iter().next().unwrap_or_default())
}
