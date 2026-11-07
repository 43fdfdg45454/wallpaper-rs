//! Certificate authorities the user installed on the phone, trusted beside the built-in roots.

use std::fs;
use std::path::Path;
use std::sync::OnceLock;

static ADDED: OnceLock<Vec<Vec<u8>>> = OnceLock::new();

/// Reads `path`, where each DER certificate follows its length as four big-endian bytes. Only the
/// first call counts; a missing or cut file adds what it holds whole.
pub fn load_added(path: &Path) {
    let bytes = fs::read(path).unwrap_or_default();
    let _ = ADDED.set(split(&bytes));
}

pub fn added() -> &'static [Vec<u8>] {
    ADDED.get().map_or(&[], Vec::as_slice)
}

fn split(mut bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut certs = Vec::new();
    while let Some((len, rest)) = bytes.split_first_chunk::<4>() {
        let len = u32::from_be_bytes(*len) as usize;
        let Some((cert, rest)) = rest.split_at_checked(len) else { break };
        certs.push(cert.to_vec());
        bytes = rest;
    }
    certs
}

#[cfg(test)]
mod tests {
    use super::split;

    #[test]
    fn splits_length_prefixed_certificates_and_drops_a_cut_one() {
        let bytes = [&[0, 0, 0, 2, 7, 8][..], &[0, 0, 0, 1, 9], &[0, 0, 0, 5, 1]].concat();
        assert_eq!(split(&bytes), vec![vec![7, 8], vec![9]]);
        assert!(split(&[0, 0]).is_empty());
    }
}
