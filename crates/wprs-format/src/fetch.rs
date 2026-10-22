use crate::repo::hex64;
use crate::store::State;
use crate::validate::within;
use crate::{Error, Result, Root, require};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

pub trait Fetch {
    fn get(&self, base: &str, path: &str, max: u64) -> io::Result<Option<Vec<u8>>>;

    fn copy(&self, base: &str, path: &str, max: u64, out: &mut dyn Write) -> io::Result<bool> {
        let Some(bytes) = self.get(base, path, max)? else { return Ok(false) };
        out.write_all(&bytes).map(|()| true)
    }
}

pub struct FileFetch;

impl Fetch for FileFetch {
    fn get(&self, base: &str, path: &str, max: u64) -> io::Result<Option<Vec<u8>>> {
        let root = base.strip_prefix("file://").unwrap_or(base);
        match File::open(Path::new(root).join(path)) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(max.saturating_add(1)).read_to_end(&mut bytes)?;
                Ok(Some(bytes))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    pub base: String,
    pub fingerprint: Option<String>,
    pub repo_id: Option<String>,
}

impl Anchor {
    pub fn parse(url: &str) -> Result<Self> {
        let (base, query) = url.split_once('?').unwrap_or((url, ""));
        let mut anchor =
            Anchor { base: format!("{}/", base.trim_end_matches('/')), fingerprint: None, repo_id: None };
        for (key, value) in query.split('&').filter_map(|pair| pair.split_once('=')) {
            let slot = match key {
                "fingerprint" => &mut anchor.fingerprint,
                "repo_id" => &mut anchor.repo_id,
                _ => continue,
            };
            let value = value.to_ascii_lowercase();
            require(hex64(&value), key, "expected 64 hex digits")?;
            *slot = Some(value);
        }
        let anchored = anchor.fingerprint.is_some() || anchor.repo_id.is_some();
        require(anchored, "url", "needs a fingerprint or repo_id query parameter")?;
        Ok(anchor)
    }
}

pub(crate) fn bases(state: &State, root: &Root) -> Vec<String> {
    let mut bases = state.bases.clone();
    for mirror in root.mirrors.iter().map(|m| format!("{}/", m.trim_end_matches('/'))) {
        if !bases.contains(&mirror) {
            bases.push(mirror);
        }
    }
    bases
}

pub(crate) fn first<T>(
    bases: &[String],
    mut attempt: impl FnMut(&str) -> Result<Option<T>>,
) -> Result<Option<T>> {
    let (mut missing, mut error) = (false, None);
    for base in bases {
        match attempt(base) {
            Ok(Some(value)) => return Ok(Some(value)),
            Ok(None) => missing = true,
            Err(Error::Io(e)) if e.kind() == io::ErrorKind::Interrupted => return Err(e.into()),
            Err(e) => error = Some(e),
        }
    }
    error.filter(|_| !missing).map_or(Ok(None), Err)
}

pub(crate) fn get(fetch: &dyn Fetch, base: &str, path: &str, max: u64) -> Result<Option<Vec<u8>>> {
    let bytes = fetch.get(base, path, max)?;
    bytes.as_ref().map_or(Ok(()), |b| within("download size", b.len() as u64, max))?;
    Ok(bytes)
}
