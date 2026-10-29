use crate::daemon::Daemon;
use crate::error::{Error, Result, invalid, not_found};
use crate::jobs::{Ctl, now};
use crate::thumbs::Req;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wprs_core::dbus::uri_scheme;
use wprs_core::model::{AnalyzeReport, JobKind, ScanOutput, SourceScan};
use wprs_library::{Item, ScanReport};

pub fn local(path: &str) -> Result<PathBuf> {
    match uri_scheme(path) {
        Some(_) => Err(invalid(format!("{path} is a URI; give a local path"))),
        None if !Path::new(path).is_absolute() => Err(invalid(format!("{path} is not an absolute path"))),
        None => Ok(path.into()),
    }
}

fn hidden(path: &Path) -> bool {
    path.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
}

fn scanned(source: Option<String>, r: ScanReport) -> SourceScan {
    SourceScan { source, added: r.added, updated: r.updated, moved: r.moved, missing: r.missing }
}

impl Daemon {
    pub fn add_source(self: &Arc<Self>, path: &str, recursive: bool) -> Result<(String, u64)> {
        let given = local(path)?;
        let dir = fs::canonicalize(&given).ok().filter(|dir| dir.is_dir());
        let dir = dir.ok_or_else(|| invalid(format!("{path} is not an existing directory")))?;
        for data in [&self.store.cache, &self.store.data] {
            let data = fs::canonicalize(data).unwrap_or_else(|_| data.clone());
            let contains = recursive && data.strip_prefix(&dir).is_ok_and(|rest| !hidden(rest));
            if dir.starts_with(&data) || contains {
                return Err(invalid(format!("{path} contains wallpaper-rs data")));
            }
        }
        let uid = self.store.write(|lib| {
            for s in lib.sources()? {
                if s.path == dir
                    || (s.recursive && dir.starts_with(&s.path))
                    || (recursive && s.path.starts_with(&dir))
                {
                    return Err(invalid(format!("{path} overlaps the source {}", s.path.display())));
                }
            }
            Ok(lib.add_folder(&dir, recursive)?)
        })?;
        self.library_changed(Vec::new());
        self.watch_sources();
        Ok((uid.clone(), self.scan_job(&uid)))
    }

    pub fn provider_of(&self, path: &Path) -> Option<String> {
        let providers = self.store.cache.join("providers");
        let providers = std::fs::canonicalize(&providers).unwrap_or(providers);
        let own = path.parent().is_some_and(|parent| parent == providers);
        own.then(|| path.file_name().map(|name| name.to_string_lossy().into_owned())).flatten()
    }

    pub fn remove_source(&self, uid: &str) -> Result<()> {
        let sources = self.store.read(|lib| lib.sources())?;
        let source = sources.into_iter().find(|s| s.uid == uid);
        let source = source.ok_or_else(|| not_found(format!("source {uid:?} not found")))?;
        if let Some(id) = self.provider_of(&source.path) {
            return Err(invalid(format!("belongs to provider \"{id}\"; remove the provider")));
        }
        self.jobs.find(JobKind::Scan, uid).into_iter().try_for_each(|job| self.cancel_job(job))?;
        let removed = self.store.write(|lib| lib.remove_source(uid))?;
        self.forget_files(&removed);
        self.library_changed(Vec::new());
        self.watch_sources();
        Ok(())
    }

    pub fn rescan(self: &Arc<Self>, source: &str) -> Result<u64> {
        if !source.is_empty() && !self.store.read(|lib| lib.sources())?.iter().any(|s| s.uid == source) {
            return Err(not_found(format!("source {source:?} not found")));
        }
        Ok(self.scan_job(source))
    }

    pub fn scan_job(self: &Arc<Self>, target: &str) -> u64 {
        let source = target.to_owned();
        self.start_job(JobKind::Scan, target, move |d, ctl| d.scan(&source, ctl))
    }

    fn scan(self: &Arc<Self>, target: &str, ctl: &Ctl) -> Result<Value> {
        let mut lib = self.store.connect()?;
        let sources = match target {
            "" => lib.sources()?.into_iter().map(|s| s.uid).collect(),
            uid => vec![uid.to_owned()],
        };
        let (mut reports, mut base, mut stored) = (Vec::new(), (0, 0), false);
        for uid in sources {
            let mut seen = (0, 0);
            let mut progress = |done: u32, total: u32| {
                seen = (u64::from(done), u64::from(total));
                ctl.progress(base.0 + seen.0, base.1 + seen.1);
                if ctl.check().is_ok() { ControlFlow::Continue(()) } else { ControlFlow::Break(()) }
            };
            let report = lib.scan_with(&uid, now(), &mut progress);
            self.library_changed(Vec::new());
            let report = match report {
                Err(wprs_library::Error::Cancelled) => {
                    stored = seen.0 > 0;
                    break;
                }
                Err(_) if target.is_empty() => continue,
                report => report?,
            };
            base = (base.0 + seen.0, base.1 + seen.1);
            reports.push(scanned(Some(uid), report));
        }
        if target.is_empty() && ctl.check().is_ok() {
            stored |= self.offer_examples();
            reports.push(scanned(None, lib.check_loose(now())?));
            self.library_changed(Vec::new());
        }
        if stored || reports.iter().any(|r| r.added + r.updated + r.moved > 0) {
            self.analyze_job();
        }
        Ok(serde_json::to_value(ScanOutput { reports }).unwrap_or_default())
    }

    pub fn analyze_job(self: &Arc<Self>) -> u64 {
        self.start_job(JobKind::Analyze, "", |d, ctl| d.analyze(ctl))
    }

    fn analyze(&self, ctl: &Ctl) -> Result<Value> {
        let (mut tried, mut analyzed, mut failed) = (HashSet::new(), 0, 0);
        let mut total = self.store.read(|lib| lib.unanalyzed(u32::MAX))?.len() as u64;
        loop {
            let limit = tried.len() as u32 + 256;
            let found = self.store.read(|lib| lib.unanalyzed(limit))?;
            let more = found.len() as u32 == limit;
            let fresh = |i: &Item| tried.insert(i.uid.clone()) && !self.thumbs.later(i);
            let batch: Vec<Item> = found.into_iter().filter(fresh).take(256).collect();
            match (batch.is_empty(), more) {
                (true, true) => continue,
                (true, false) => break,
                _ => {}
            }
            total = total.max(tried.len() as u64);
            let (tx, rx) = std::sync::mpsc::channel();
            batch.into_iter().for_each(|item| self.thumbs.queue(item, Req::Analyze(tx.clone())));
            drop(tx);
            for ok in rx {
                (analyzed, failed) = if ok { (analyzed + 1, failed) } else { (analyzed, failed + 1) };
                ctl.progress(u64::from(analyzed + failed), total);
                if ctl.check().is_err() {
                    self.thumbs.forget_analysis();
                    return Err(Error::Cancelled);
                }
            }
        }
        Ok(serde_json::to_value(AnalyzeReport { analyzed, failed }).unwrap_or_default())
    }
}
