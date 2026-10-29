use crate::daemon::Daemon;
use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use wprs_format::Installed;

pub(crate) type Stamp = (i64, i64, u64);
pub(crate) type Keys = HashMap<PathBuf, (Stamp, String)>;

fn named(sha256: &str, signer: Option<&str>) -> String {
    signer.map_or_else(|| format!("sha256:{sha256}"), |fingerprint| format!("key:{fingerprint}"))
}

fn known(keys: &Keys, installed: &[Installed], wpak: &Path, stamp: Stamp) -> Option<String> {
    let record = installed.iter().find(|i| i.path == wpak && i.size == stamp.2);
    let cached = || keys.get(wpak).filter(|(s, _)| *s == stamp).map(|(_, key)| key.clone());
    record.map(|i| named(&i.sha256, i.signer.as_deref())).or_else(cached)
}

pub(crate) fn draft_key(dir: &Path, document: &str, scripts: &[(String, String)], trusted: bool) -> String {
    let folder = dir.as_os_str().as_encoded_bytes();
    let hash = |bytes: &[u8]| wprs_format::sha256(bytes).unwrap_or_default();
    if trusted {
        return format!("folder:{}", hash(folder));
    }
    let parts = scripts.iter().flat_map(|(path, source)| [path.as_bytes(), source.as_bytes()]);
    let content = [folder, document.as_bytes()].into_iter().chain(parts);
    let content = content.flat_map(|part| [&(part.len() as u64).to_le_bytes()[..], part].concat());
    format!("draft:{}", hash(&content.collect::<Vec<u8>>()))
}

impl Daemon {
    pub(crate) fn package_key(&self, wpak: &Path, meta: &std::fs::Metadata) -> anyhow::Result<String> {
        let stamp = (meta.mtime(), meta.mtime_nsec(), meta.len());
        let found = {
            let st = self.state();
            known(&st.keys, &st.installed, wpak, stamp)
        };
        if let Some(key) = found {
            return Ok(key);
        }
        let sha256 = wprs_format::sha256_file(wpak)?;
        let mut st = self.state();
        let signer = st.installed.iter().find(|i| i.sha256 == sha256).and_then(|i| i.signer.clone());
        let key = named(&sha256, signer.as_deref());
        st.keys.insert(wpak.to_owned(), (stamp, key.clone()));
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed(path: &str, size: u64, signer: Option<&str>) -> Installed {
        let (repo_id, id, version, revoked) = ("r".into(), "org.example.sea".into(), "1.0.0".into(), None);
        let (sha256, path, signer) = ("ab".into(), PathBuf::from(path), signer.map(str::to_owned));
        Installed { repo_id, id, version, sha256, size, path, signer, revoked }
    }

    #[test]
    fn package_keys_are_found_without_reading_the_file_again() {
        let records = [installed("/p/signed.wpak", 9, Some("fp")), installed("/p/plain.wpak", 5, None)];
        let keys = Keys::from([(PathBuf::from("/l/loose.wpak"), ((1, 2, 3), "sha256:cd".to_owned()))]);
        let key = |path: &str, stamp| known(&keys, &records, Path::new(path), stamp);
        assert_eq!(key("/p/signed.wpak", (0, 0, 9)).as_deref(), Some("key:fp"));
        assert_eq!(key("/p/plain.wpak", (0, 0, 5)).as_deref(), Some("sha256:ab"));
        assert_eq!(key("/p/signed.wpak", (0, 0, 8)), None, "a changed installed file kept its key");
        assert_eq!(key("/l/loose.wpak", (1, 2, 3)).as_deref(), Some("sha256:cd"));
        assert_eq!(key("/l/loose.wpak", (1, 3, 3)), None, "a rewritten file kept its key");
    }

    #[test]
    fn only_trusted_previews_keep_a_grant_across_edits() {
        let (dir, scripts) = (Path::new("/s/sea"), vec![("a.js".to_owned(), "audio".to_owned())]);
        let key = |document, trusted| draft_key(dir, document, &scripts, trusted);
        assert_eq!(key("{}", true), key("{\"x\":1}", true));
        assert!(key("{}", true).starts_with("folder:") && key("{}", false).starts_with("draft:"));
        assert_ne!(key("{}", false), key("{\"x\":1}", false), "another document got the grant");
        let other = draft_key(dir, "{}", &[("a.js".to_owned(), "cursor".to_owned())], false);
        assert_ne!(key("{}", false), other, "another script got the grant");
    }
}
