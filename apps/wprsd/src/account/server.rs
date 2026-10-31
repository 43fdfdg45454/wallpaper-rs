use super::{block_on, sync_error};
use crate::daemon::Daemon;
use crate::error::{Error, Result, failed, invalid};
use crate::jobs::Ctl;
use reqwest::Url;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wprs_core::dbus::error::EXISTS;
use wprs_core::model::JobKind;
use wprs_sync::{Api, check_url, peek, valid_pin};

pub fn exists(message: &str) -> Error {
    Error::Named(EXISTS, message.into())
}

pub fn same_origin(a: &str, b: &str) -> bool {
    let origin = |url: &str| Url::parse(url).ok().map(|u| u.origin());
    origin(a).is_some() && origin(a) == origin(b)
}

pub fn pause(ctl: &Ctl, wait: Duration) -> Result<()> {
    let until = Instant::now() + wait;
    while Instant::now() < until {
        ctl.check()?;
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}

impl Daemon {
    pub fn add_server(self: &Arc<Self>, url: &str, pin: Option<&str>) -> Result<u64> {
        let url = check_url(url).map_err(sync_error)?;
        let pin =
            pin.map(|p| p.chars().filter(char::is_ascii_hexdigit).collect::<String>().to_ascii_lowercase());
        if pin.as_deref().is_some_and(|p| !valid_pin(p)) {
            return Err(invalid("a certificate fingerprint has 64 hex digits"));
        }
        if !self.saved().url.is_empty() {
            return Err(exists("a server is already added; remove it first"));
        }
        Ok(self.start_job(JobKind::Account, &url.clone(), move |d, _| d.discover(&url, pin)))
    }

    fn discover(&self, url: &str, pin: Option<String>) -> Result<Value> {
        let api = Api::pinned(url, pin.as_deref()).map_err(sync_error)?;
        let found = match block_on(api.get("/.well-known/wprs-server")) {
            Err(wprs_sync::Error::Untrusted) if pin.is_none() => {
                let seen = block_on(peek(url)).map_err(sync_error)?;
                return Err(failed(format!(
                    "this server uses its own certificate with the fingerprint {seen}; \
                     add it with that fingerprint only if the server shows the same one"
                )));
            }
            found => found.map_err(sync_error)?,
        };
        if !found["api"].as_array().is_some_and(|api| api.contains(&json!("v1"))) {
            return Err(failed("this address does not serve a wallpaper-rs server"));
        }
        let repo = found["repository"]["url"].as_str().filter(|repo| same_origin(repo, url));
        let name = found["name"].as_str().unwrap_or_default().chars().filter(|c| !c.is_control()).take(128);
        self.update(|s| {
            (s.url, s.name, s.pin) = (url.to_owned(), name.collect(), pin);
            s.repo_url = repo.map(String::from);
            s.repo_id = found["repository"]["repo_id"].as_str().filter(|_| repo.is_some()).map(String::from);
        })?;
        Ok(json!({"name": found["name"], "registration": found["registration"], "repository": repo}))
    }
}
