use crate::daemon::Daemon;
use crate::error::{Result, failed, invalid, not_found};
use crate::items::reply;
use crate::jobs::now;
use crate::repos::{fetcher, newer};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;
use wprs_core::model::{InstalledPackage, InstalledReply, JobKind, MODEL};
use wprs_format::{Fetch, Installed};

struct Dry;

impl Fetch for Dry {
    fn get(&self, _: &str, _: &str, _: u64) -> io::Result<Option<Vec<u8>>> {
        Err(io::ErrorKind::WouldBlock.into())
    }
}

struct Progress<F, P>(F, P);

struct Counted<'a>(&'a mut dyn Write, &'a dyn Fn(u64), u64);

impl Write for Counted<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.0.write(buf)?;
        self.2 += written as u64;
        (self.1)(self.2);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl<F: Fetch, P: Fn(u64)> Fetch for Progress<F, P> {
    fn get(&self, base: &str, path: &str, max: u64) -> io::Result<Option<Vec<u8>>> {
        self.0.get(base, path, max)
    }

    fn copy(&self, base: &str, path: &str, max: u64, out: &mut dyn Write) -> io::Result<bool> {
        self.0.copy(base, path, max, &mut Counted(out, &self.1, 0))
    }
}

pub fn parse_spec(spec: &str) -> Result<(Option<&str>, &str, Option<&str>)> {
    let (repo, rest) = spec.rsplit_once('/').map_or((None, spec), |(repo, rest)| (Some(repo), rest));
    let (id, version) = rest.split_once('@').map_or((rest, None), |(id, version)| (id, Some(version)));
    let token = |t: &str, extra: &str| {
        !t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric() || extra.contains(c))
    };
    let repo_ok = repo.is_none_or(|r| r.len() >= 8 && r.chars().all(|c| c.is_ascii_hexdigit()));
    if repo_ok && token(id, "._-") && id.contains('.') && version.is_none_or(|v| token(v, ".+-")) {
        return Ok((repo, id, version));
    }
    Err(invalid(format!("{spec:?} is not [REPO/]ID[@VERSION]")))
}

impl Daemon {
    pub fn shown_on(&self, path: &Path) -> Result<()> {
        let st = self.state();
        let connected = st.outputs.keys().filter(|o| st.config.resolve(o).image.as_deref() == Some(path));
        let configured =
            st.config.outputs.iter().filter(|(_, o)| o.image.as_deref() == Some(path)).map(|(n, _)| n);
        let default = (st.config.default.image.as_deref() == Some(path)).then_some("every output");
        match connected.chain(configured).map(String::as_str).chain(default).next() {
            Some(name) => Err(failed(format!("the package is shown on {name}; change the wallpaper first"))),
            None => Ok(()),
        }
    }

    pub fn install(self: &Arc<Self>, spec: &str, allow_stale: bool, allow_downgrade: bool) -> Result<u64> {
        let (repo, id, version) = parse_spec(spec)?;
        let repo = repo.map(|r| self.repo_id(r)).transpose()?;
        let at = version.map_or(String::new(), |v| format!("@{v}"));
        let full = |repo: Option<&str>| repo.map_or(String::new(), |r| format!("{r}/")) + id + &at;
        let data = self.store.data.clone();
        let checked =
            wprs_format::install(&data, &Dry, &full(repo.as_deref()), now(), allow_stale, allow_downgrade);
        let done = match checked {
            Ok(done) => Some(done),
            Err(wprs_format::Error::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => {
                std::fs::remove_dir(data.join("packages").join(id)).ok();
                None
            }
            Err(wprs_format::Error::Invalid { field, reason }) if field == "id" => {
                return Err(invalid(format!("{id} {reason}")));
            }
            Err(wprs_format::Error::Expired(_)) => {
                return Err(failed("root expired: possible freeze attack"));
            }
            Err(e) => return Err(e.into()),
        };
        self.library_ready()?;
        let offers = wprs_format::browse(&data, id)?;
        let offer = offers.into_iter().find(|o| o.id == id && repo.as_ref().is_none_or(|r| *r == o.repo_id));
        let (repo_id, version, size) = match (&done, offer) {
            (Some(done), _) => (done.repo_id.clone(), done.version.clone(), done.size),
            (None, Some(offer)) => {
                let version = version.map_or_else(|| offer.release.version.clone(), String::from);
                let size = if version == offer.release.version { offer.release.size } else { 0 };
                (offer.repo_id, version, size)
            }
            (None, None) => return Err(not_found(format!("{spec} is not offered"))),
        };
        let spec = full(Some(&repo_id));
        Ok(self.start_job(JobKind::Install, &format!("{repo_id}/{id}@{version}"), move |d, ctl| {
            let record = match done {
                Some(done) => done,
                None => {
                    let fetch = Progress(fetcher(ctl)?, |done| ctl.progress(done, size));
                    wprs_format::install(&d.store.data, &fetch, &spec, now(), allow_stale, allow_downgrade)?
                }
            };
            let _turn = d.jobs.wait_turn(JobKind::Scan);
            let uid = d.link(&record)?;
            tracing::info!(repo = &record.repo_id[..8], "package installed");
            d.reload_installed();
            Ok(serde_json::to_value(InstalledPackage { item: Some(uid), ..d.package_model(&record, None) })
                .unwrap_or_default())
        }))
    }

    fn link(self: &Arc<Self>, record: &Installed) -> Result<String> {
        let path = |p: &Path| p.to_str().map(String::from).unwrap_or_default();
        let siblings: Vec<String> = wprs_format::installed(&self.store.data)?
            .into_iter()
            .filter(|i| i.repo_id == record.repo_id && i.id == record.id && i.version != record.version)
            .map(|i| path(&i.path))
            .collect();
        let (uid, old) = self.store.write(|lib| {
            if let Ok(uid) = lib.resolve_item(&path(&record.path)) {
                return Ok((uid, None));
            }
            match siblings.iter().find_map(|p| lib.resolve_item(p).ok()) {
                Some(uid) => {
                    let old = lib.item(&uid)?;
                    lib.relink_item(&uid, &record.path, now())?;
                    Ok::<_, wprs_library::Error>((uid, Some(old)))
                }
                None => Ok((lib.import_file(&record.path, now())?, None)),
            }
        })?;
        self.forget_files(old.as_slice());
        self.library_changed(vec![uid.clone()]);
        self.analyze_job();
        Ok(uid)
    }

    pub fn uninstall(&self, id: &str, version: &str) -> Result<()> {
        self.library_ready()?;
        let data = &self.store.data;
        let all = wprs_format::installed(data)?;
        let gone: Vec<&Installed> =
            all.iter().filter(|i| i.id == id && (version.is_empty() || i.version == version)).collect();
        if gone.is_empty() {
            return Err(not_found(format!("{id} {version} is not installed").replace("  ", " ")));
        }
        gone.iter().try_for_each(|i| self.shown_on(&i.path))?;
        let resolve = |i: &&Installed| {
            self.store.read(|lib| lib.resolve_item(i.path.to_str().unwrap_or_default())).ok()
        };
        let linked = gone.iter().find_map(resolve);
        wprs_format::remove(data, id, Some(version).filter(|v| !v.is_empty()))?;
        if let Some(uid) = linked {
            let rest = wprs_format::installed(data)?.into_iter().filter(|i| i.id == id);
            let newest = rest.max_by_key(|i| semver::Version::parse(&i.version).ok());
            let old = self.store.read(|lib| lib.item(&uid))?;
            let removed = match newest {
                Some(newest) => {
                    self.store.write(|lib| lib.relink_item(&uid, &newest.path, now())).map(|()| vec![old])?
                }
                None => self.store.write(|lib| lib.remove_items(&[uid.as_str()]))?,
            };
            self.forget_files(&removed);
            self.library_changed(vec![uid]);
        }
        tracing::info!("package uninstalled");
        self.reload_installed();
        Ok(())
    }

    pub fn list_installed(&self) -> Result<String> {
        let serial = self.changes.serial();
        let list = wprs_format::installed(&self.store.data)?;
        let latest: HashMap<(String, String), String> = wprs_format::browse(&self.store.data, "")?
            .into_iter()
            .map(|item| ((item.repo_id, item.id), item.release.version))
            .collect();
        let model = |record: &Installed| {
            let newest =
                list.iter().filter(|i| i.id == record.id).all(|i| !newer(&i.version, &record.version));
            let available = latest.get(&(record.repo_id.clone(), record.id.clone()));
            let update = available.filter(|v| newest && newer(v, &record.version)).cloned();
            self.package_model(record, update)
        };
        reply(InstalledReply { model: MODEL, serial, packages: list.iter().map(model).collect() })
    }

    pub fn package_model(&self, record: &Installed, update: Option<String>) -> InstalledPackage {
        let item = record.path.to_str().and_then(|p| self.store.read(|lib| lib.resolve_item(p)).ok());
        let revoked = record.revoked.and_then(|r| serde_json::from_value(serde_json::to_value(r).ok()?).ok());
        InstalledPackage {
            repo_id: record.repo_id.clone(),
            id: record.id.clone(),
            version: record.version.clone(),
            sha256: record.sha256.clone(),
            size: record.size,
            signer: record.signer.clone(),
            revoked,
            item,
            update,
        }
    }
}

#[cfg(test)]
mod tests;
