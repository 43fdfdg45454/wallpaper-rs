use crate::config::Config;
use crate::db::now;
use anyhow::{Context, ensure};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use wprs_format::{
    Added, Entry, Keypair, Published, Reason, RootStatus, Settings, Signer, fingerprint, keygen, settings,
};

pub struct Repo {
    pub dir: PathBuf,
    key: PathBuf,
    lock: Mutex<()>,
}

impl Repo {
    pub fn new(data: &Path) -> Repo {
        Repo { dir: data.join("repo"), key: data.join("keys/online.key"), lock: Mutex::new(()) }
    }

    pub fn ready(&self) -> bool {
        self.dir.join("root/1.json").is_file() && self.key.is_file()
    }

    pub fn add(&self, wpak: &Path, now: i64) -> wprs_format::Result<Added> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        wprs_format::add(&self.dir, wpak, None, now / 1000)
    }

    pub fn revoke(&self, id: &str, version: &str, reason: Reason, message: &str) -> wprs_format::Result<()> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        wprs_format::revoke(&self.dir, id, Some(version), reason, message)?;
        let catalog: Value = serde_json::from_slice(&fs::read(self.dir.join("catalog.json"))?)?;
        let list = |v: &Value, key: &str| v[key].as_array().cloned().unwrap_or_default();
        let revoked = list(&catalog, "revoked");
        let withdrawn = |id: &Value, v: &Value| {
            revoked.iter().any(|r| r["id"] == *id && list(r, "versions").iter().any(|x| x == v || x == "*"))
        };
        let shown: HashSet<String> = list(&catalog, "packages")
            .iter()
            .flat_map(|l| list(l, "versions").into_iter().filter(|r| !withdrawn(&l["id"], &r["version"])))
            .filter_map(|r| {
                Some(format!("{}.{}", r["preview"]["sha256"].as_str()?, r["preview"]["ext"].as_str()?))
            })
            .collect();
        for item in fs::read_dir(self.dir.join("previews")).into_iter().flatten().flatten() {
            if !shown.contains(&*item.file_name().to_string_lossy()) {
                let _ = fs::remove_file(item.path());
            }
        }
        Ok(())
    }

    pub fn hide(&self, id: &str, hidden: bool) -> wprs_format::Result<()> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        wprs_format::hide(&self.dir, id, hidden)
    }

    pub fn restore(&self, id: &str, version: &str, held: &Path, now: i64) -> anyhow::Result<()> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let dest = self.dir.join("packages").join(id).join(format!("{version}.wpak"));
        wprs_format::add(&self.dir, held, None, now / 1000)?;
        DirBuilder::new().recursive(true).mode(0o700).create(dest.parent().context("bad path")?)?;
        fs::copy(held, &dest)?;
        Ok(wprs_format::unrevoke(&self.dir, id, version)?)
    }

    pub fn publish(&self, days: u32, now: i64) -> anyhow::Result<Published> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let signer = Signer::new(&fs::read_to_string(&self.key)?, None)?;
        let days = days.min(settings(&self.dir)?.entry_max_days);
        Ok(wprs_format::publish(&self.dir, &signer, days, now / 1000)?)
    }

    pub fn authors(&self) -> Option<(Vec<String>, bool, bool)> {
        let s = settings(&self.dir).ok()?;
        let keys = s.authors.into_values().flatten().filter_map(|k| fingerprint(&k).ok()).collect();
        Some((keys, s.require_author_signature, s.online_authors))
    }

    pub fn online(&self) -> bool {
        self.authors().is_some_and(|(_, _, online)| online)
    }

    pub fn register(&self, user: i64, key: &str) -> wprs_format::Result<String> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        wprs_format::register_author(&self.dir, &author(user), key)
    }

    pub fn rotate(&self, user: i64, from: &str, key: &str, sig: &str) -> wprs_format::Result<String> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        wprs_format::rotate_author(&self.dir, &author(user), from, key, sig)
    }

    pub fn drop_key(&self, user: i64, fp: &str) -> wprs_format::Result<()> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        wprs_format::drop_author_key(&self.dir, &author(user), fp)
    }

    pub fn info(&self, cfg: &Config) -> Option<Value> {
        let root = wprs_format::status(&self.dir).ok()?;
        let latest =
            fs::read_to_string(self.dir.join("entry/latest")).ok().and_then(|t| t.trim().parse::<u64>().ok());
        let entry = latest.and_then(|n| fs::read(self.dir.join(format!("entry/{n}.json"))).ok());
        let entry: Option<Entry> = entry.and_then(|bytes| serde_json::from_slice(&bytes).ok());
        let (entry_version, entry_expires) =
            (entry.as_ref().map(|e| e.version), entry.map(|e| e.expires.to_rfc3339()));
        Some(json!({
            "repo_id": root.repo_id, "root_version": root.version, "root_signed": root.complete,
            "url": cfg.url(&format!("/repo/?{}", root.query)), "online_key": self.key.is_file(),
            "entry_version": entry_version, "entry_expires": entry_expires,
        }))
    }
}

pub fn author(user: i64) -> String {
    format!("u{user}")
}

pub fn init(cfg: &Config, name: &str, require_signature: bool, open: bool) -> anyhow::Result<Keypair> {
    let repo = Repo::new(&cfg.data);
    ensure!(!repo.dir.join("root/1.json").exists(), "a repository already exists");
    let (root, online) = (keygen(None)?, keygen(None)?);
    let (root_keys, online_keys) = (vec![root.public.clone()], vec![online.public.clone()]);
    let settings = Settings {
        name: name.into(),
        root_keys,
        online_keys,
        require_author_signature: require_signature,
        online_authors: open,
        ..Settings::default()
    };
    wprs_format::init(&repo.dir, &settings, now() / 1000)?;
    wprs_format::sign_root(&repo.dir, &Signer::new(&root.secret, None)?)?;
    store_key(&repo.key, &online.secret)?;
    Ok(root)
}

pub fn import(cfg: &Config, src: &Path, online: Option<&Path>) -> anyhow::Result<RootStatus> {
    let repo = Repo::new(&cfg.data);
    DirBuilder::new().recursive(true).mode(0o700).create(repo.dir.join("root"))?;
    for item in fs::read_dir(src.join("root")).context("the source has no root folder")? {
        let item = item?;
        let (dest, bytes) = (repo.dir.join("root").join(item.file_name()), fs::read(item.path())?);
        match fs::read(&dest) {
            Ok(stored) => ensure!(stored == bytes, "{} differs from the stored copy", dest.display()),
            Err(e) if e.kind() == ErrorKind::NotFound => fs::write(&dest, bytes)?,
            Err(e) => return Err(e.into()),
        }
    }
    if !repo.dir.join("catalog.json").exists() {
        fs::copy(src.join("catalog.json"), repo.dir.join("catalog.json"))
            .context("the source has no catalog.json")?;
    }
    let status = wprs_format::status(&repo.dir)?;
    ensure!(status.complete, "root {} is not signed by enough root keys", status.version);
    if let Some(path) = online {
        let secret = fs::read_to_string(path)?;
        let signer = Signer::new(&secret, None).context("expected an unencrypted minisign secret key")?;
        let keys = settings(&repo.dir)?.online_keys;
        ensure!(
            keys.iter().any(|k| fingerprint(k).is_ok_and(|f| f == signer.fingerprint)),
            "the key is not the online key of the root"
        );
        store_key(&repo.key, &secret)?;
    }
    Ok(status)
}

pub fn status(cfg: &Config) -> anyhow::Result<Value> {
    Repo::new(&cfg.data).info(cfg).context("no repository yet: run wprs-server repo init")
}

fn store_key(path: &Path, secret: &str) -> anyhow::Result<()> {
    DirBuilder::new().recursive(true).mode(0o700).create(path.parent().context("bad key path")?)?;
    let mut file = OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?;
    Ok(file.write_all(secret.as_bytes())?)
}
