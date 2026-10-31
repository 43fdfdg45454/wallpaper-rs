use crate::{Error, Item, Media, Result, invalid, path_of};
use reqwest::header::{ETAG, HeaderName, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use reqwest::{RequestBuilder, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{File, FileTimes, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;
use tokio::io::AsyncWriteExt;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x100_0000_01b3;

#[derive(Debug, Clone)]
pub struct Cache {
    dir: PathBuf,
    limit: u64,
    room: Arc<AtomicU64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Validators {
    etag: Option<String>,
    modified: Option<String>,
}

impl Cache {
    pub fn new(dir: impl Into<PathBuf>, limit: u64) -> Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        std::fs::set_permissions(&dir, Permissions::from_mode(0o700))?;
        Ok(Self { dir, limit, room: Arc::new(AtomicU64::new(u64::MAX)) })
    }

    pub fn allow(&self, bytes: u64) {
        self.room.store(bytes, Ordering::Relaxed);
    }

    pub fn path(&self, item: &Item) -> PathBuf {
        let ext = [path_of(&item.url), item.title.as_str()]
            .into_iter()
            .find_map(|name| Media::detect(name).filter(|(media, _)| *media == item.media))
            .map_or_else(|| if item.media == Media::Image { "jpg" } else { "mp4" }.into(), |(_, ext)| ext);
        let hash = item.id.bytes().fold(FNV_OFFSET, |h, b| (h ^ u64::from(b)).wrapping_mul(FNV_PRIME));
        self.dir.join(format!("{hash:016x}.{ext}"))
    }

    pub async fn download(&self, item: &Item, request: RequestBuilder) -> Result<PathBuf> {
        let path = self.path(item);
        let sidecar = sidecar(&path);
        let read = || std::fs::read(&sidecar).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let known: Validators = path.exists().then(read).flatten().unwrap_or_default();
        let request = known.etag.iter().fold(request, |r, v| r.header(IF_NONE_MATCH, v));
        let request = known.modified.iter().fold(request, |r, v| r.header(IF_MODIFIED_SINCE, v));
        let response = match request.send().await.and_then(Response::error_for_status) {
            Ok(response) if response.status() == StatusCode::NOT_MODIFIED => return touch(path),
            Ok(response) if response.status().is_redirection() && !path.exists() => {
                return Err(invalid("redirect refused"));
            }
            Ok(response) if response.status().is_redirection() => return touch(path),
            Err(_) if path.exists() => return touch(path),
            response => response?,
        };
        let room = self.room.load(Ordering::Relaxed);
        if response.content_length().is_some_and(|len| len > room) {
            return Err(Error::TooLarge);
        }
        let fresh = Validators { etag: header(&response, ETAG), modified: header(&response, LAST_MODIFIED) };
        let part = path.with_extension("part");
        if let Err(error) = store(&part, response, room).await {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(error);
        }
        tokio::fs::rename(&part, &path).await?;
        let temp = sidecar.with_extension("tmp");
        let mut file = private(&temp).await?;
        file.write_all(&serde_json::to_vec(&fresh)?).await?;
        file.flush().await?;
        tokio::fs::rename(&temp, &sidecar).await?;
        Ok(path)
    }

    pub fn evict_except(&self, protected: &HashSet<PathBuf>) -> Result<Vec<PathBuf>> {
        let dir = std::fs::canonicalize(&self.dir)?;
        let real = |p: &PathBuf| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone());
        let protected: HashSet<PathBuf> = protected.iter().map(real).collect();
        let media = std::fs::read_dir(&dir)?.flatten().map(|entry| entry.path());
        let media = media.filter(|path| path.to_str().and_then(Media::detect).is_some());
        let stat = |path: PathBuf| {
            let meta = std::fs::symlink_metadata(&path).ok().filter(|meta| meta.is_file())?;
            (real(&path).parent() == Some(dir.as_path())).then_some(())?;
            Some((meta.accessed().ok()?, meta.len(), path))
        };
        let mut files: Vec<_> = media.filter_map(stat).collect();
        files.sort();
        let mut total: u64 = files.iter().map(|file| file.1).sum();
        files.pop();
        let mut removed = Vec::new();
        for (_, len, path) in files.into_iter().filter(|file| !protected.contains(&file.2)) {
            if total <= self.limit {
                break;
            }
            let _ = std::fs::remove_file(sidecar(&path));
            std::fs::remove_file(&path)?;
            total -= len;
            removed.push(path);
        }
        Ok(removed)
    }
}

async fn store(part: &Path, mut response: Response, room: u64) -> Result<()> {
    let (mut file, mut left) = (private(part).await?, room);
    while let Some(chunk) = response.chunk().await? {
        left = left.checked_sub(chunk.len() as u64).ok_or(Error::TooLarge)?;
        file.write_all(&chunk).await?;
    }
    Ok(file.sync_all().await?)
}

async fn private(path: &Path) -> std::io::Result<tokio::fs::File> {
    tokio::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path).await
}

fn touch(path: PathBuf) -> Result<PathBuf> {
    File::options().write(true).open(&path)?.set_times(FileTimes::new().set_accessed(SystemTime::now()))?;
    Ok(path)
}

fn header(response: &Response, name: HeaderName) -> Option<String> {
    response.headers().get(name)?.to_str().ok().map(Into::into)
}

fn sidecar(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".json");
    name.into()
}
