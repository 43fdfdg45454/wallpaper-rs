use crate::files::feed;
use anyhow::{Result, anyhow, bail, ensure};
use reqwest::redirect::Policy;
use reqwest::{StatusCode, header};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

pub const AGENT: &str = concat!("wallpaper-rs/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, thiserror::Error)]
#[error("the download was cancelled; it resumes next time")]
pub struct Cancelled;

pub type Progress<'a> = &'a mut dyn FnMut(u64, u64) -> bool;

fn host(url: &str) -> &str {
    url.split('/').nth(2).unwrap_or(url)
}

fn hex(hash: Sha256) -> String {
    hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

pub fn allowed() -> bool {
    let config = wprs_core::Config::default_path().and_then(|p| wprs_core::Config::load(&p));
    config.is_ok_and(|c| c.models.allow_download)
}

pub fn download(urls: &[&str], dest: &Path, bytes: u64, sha: &str, progress: Progress<'_>) -> Result<()> {
    let part = dest.with_added_extension("part");
    let redirect = Policy::custom(|a| {
        let downgrade =
            a.previous().last().is_some_and(|p| p.scheme() == "https") && a.url().scheme() != "https";
        match () {
            _ if downgrade => a.error("refused a redirect from HTTPS to HTTP"),
            _ if a.previous().len() > 10 => a.error("too many redirects"),
            _ => a.follow(),
        }
    });
    let client = reqwest::blocking::Client::builder()
        .user_agent(AGENT)
        .referer(false)
        .redirect(redirect)
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(60))
        .build()?;
    let mut failure = anyhow!("no address to download from");
    for url in urls.iter().filter(|u| !u.is_empty()) {
        match get(&client, url, &part, bytes, progress) {
            Ok(hash) if hash == sha => {
                std::fs::rename(&part, dest)?;
                dest.parent().and_then(|d| File::open(d).ok()).map(|d| d.sync_all());
                return Ok(());
            }
            Ok(_) => {
                std::fs::remove_file(&part).ok();
                failure = anyhow!("the file from {} has another SHA-256 and was deleted", host(url));
            }
            Err(e) if e.is::<Cancelled>() => return Err(e),
            Err(e) => failure = e.context(format!("cannot download from {}", host(url))),
        }
    }
    Err(failure)
}

fn get(
    client: &reqwest::blocking::Client,
    url: &str,
    part: &Path,
    bytes: u64,
    progress: Progress<'_>,
) -> Result<String> {
    let mut have = std::fs::metadata(part).map_or(0, |m| m.len());
    if have > bytes {
        std::fs::remove_file(part)?;
        have = 0;
    }
    let mut hash = Sha256::new();
    if have == bytes {
        feed(&mut hash, File::open(part)?)?;
        return Ok(hex(hash));
    }
    let request = client.get(url);
    let request = if have > 0 { request.header(header::RANGE, format!("bytes={have}-")) } else { request };
    let mut response = request.send()?.error_for_status()?;
    let range =
        response.headers().get(header::CONTENT_RANGE).and_then(|v| v.to_str().ok()).unwrap_or_default();
    let resumed =
        response.status() == StatusCode::PARTIAL_CONTENT && range.starts_with(&format!("bytes {have}-"));
    match resumed {
        true => feed(&mut hash, File::open(part)?.take(have))?,
        false => have = 0,
    }
    let length = response.content_length().unwrap_or_default();
    ensure!(have + length <= bytes, "{} offers more than {bytes} bytes", host(url));
    let mut file =
        OpenOptions::new().create(true).write(true).append(resumed).truncate(!resumed).open(part)?;
    let (mut done, mut chunk) = (have, vec![0; 1 << 16]);
    let stopped = |done| format!("the download stopped after {done} of {bytes} bytes; it resumes next time");
    loop {
        let n = match response.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                file.sync_all().ok();
                return Err(anyhow!(e).context(stopped(done)));
            }
        };
        done += n as u64;
        if done > bytes {
            drop(file);
            std::fs::remove_file(part).ok();
            bail!("{} sent more than {bytes} bytes; the file was deleted", host(url));
        }
        file.write_all(&chunk[..n])?;
        hash.update(&chunk[..n]);
        if !progress(done, bytes) {
            file.sync_all().ok();
            return Err(Cancelled.into());
        }
    }
    file.sync_all()?;
    ensure!(done == bytes, stopped(done));
    Ok(hex(hash))
}
