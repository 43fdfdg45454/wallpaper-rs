use super::Repo;
use crate::input::now;
use crate::keys::{keygen, keys, signer};
use crate::show::{either, lines, list};
use crate::usage;
use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use wprs_format::{
    Reason, RootStatus, Settings, add, init, publish, revoke, revoke_key, rotate_root, settings, sign_root,
    status, verify,
};
use wprs_providers::HttpFetch;

pub fn run(command: Repo) -> Result<()> {
    match command {
        Repo::Keygen { name, no_password } => println!("{}", keygen(&name, no_password)?),
        Repo::Init { dir, name, root_keys, online_keys, policy, json } => {
            let (root_keys, online_keys) = (keys(&root_keys)?, keys(&online_keys)?);
            let mut settings = Settings { name, root_keys, online_keys, ..Settings::default() };
            policy.apply(&mut settings)?;
            root(&init(&dir, &settings, now()?)?, json)?;
            eprintln!("sign the root next: wprs-ctl repo sign-root {} --key <root key>", dir.display())
        }
        Repo::SignRoot { dir, key, json } => root(&sign_root(&dir, &signer(&key)?)?, json)?,
        Repo::Status { dir, json } => root(&status(&dir)?, json)?,
        Repo::Add { dir, files, author, json } => {
            let remote = [&dir].into_iter().chain(&files).any(|p| p.to_string_lossy().contains("://"));
            if remote {
                return Err(usage("use wprs-ctl remote add to add a repository to this computer"));
            }
            for file in files {
                let added = add(&dir, &file, author.as_deref(), now()?)?;
                let (release, author) = (&added.release, added.author.as_deref().unwrap_or("-"));
                let text =
                    format!("{} {} sha256={} author={author}", added.id, release.version, release.sha256);
                either(&added, json, text)?
            }
        }
        Repo::Revoke { dir, id, version, reason, message } => {
            revoke(&dir, &id, version.as_deref(), self::reason(&reason)?, &message)?
        }
        Repo::RevokeKey { dir, fingerprint, reason } => {
            revoke_key(&dir, &fingerprint, self::reason(&reason)?)?
        }
        Repo::Publish(p) | Repo::Refresh(p) => {
            let published = publish(&p.dir, &signer(&p.key)?, p.days, now()?)?;
            let text = format!(
                "index {} published with {} packages; it expires {}",
                published.version, published.packages, published.expires
            );
            either(&published, p.json, text)?
        }
        Repo::RotateRoot { dir, root_keys, online_keys, removed, policy, json } => {
            let mut settings = settings(&dir)?;
            settings.root_keys = if root_keys.is_empty() { settings.root_keys } else { keys(&root_keys)? };
            settings.online_keys =
                if online_keys.is_empty() { settings.online_keys } else { keys(&online_keys)? };
            for id in removed {
                settings.authors.remove(&id).with_context(|| format!("the root lists no author {id}"))?;
            }
            policy.apply(&mut settings)?;
            root(&rotate_root(&dir, &settings, now()?)?, json)?;
            eprintln!("sign the new root with keys of the old and of the new root, then publish");
        }
        Repo::Verify { url, json } => {
            let url =
                if Path::new(&url).is_dir() { std::path::absolute(&url)?.display().to_string() } else { url };
            let info = verify(&HttpFetch::new(Arc::default())?, &url, now()?)?;
            info.warnings.iter().for_each(|warning| eprintln!("warning: {warning}"));
            let text = lines([
                ("repo_id", info.repo_id.clone()),
                ("name", info.name.clone()),
                ("hosts", list(&info.hosts)),
                ("root", format!("version {}, expires {}", info.root_version, info.root_expires)),
                ("index", format!("version {}, expires {}", info.entry_version, info.entry_expires)),
                ("packages", info.packages.to_string()),
                ("stale", info.stale.to_string()),
            ]);
            either(&info, json, text.trim_end().into())?
        }
    }
    Ok(())
}

pub(super) fn root(status: &RootStatus, json: bool) -> Result<()> {
    let text = lines([
        ("repo_id", status.repo_id.clone()),
        ("version", status.version.to_string()),
        ("signers", if status.signers.is_empty() { "-".into() } else { list(&status.signers) }),
        ("threshold", status.threshold.to_string()),
        ("complete", status.complete.to_string()),
        ("anchor", format!("?{}", status.query)),
    ]);
    either(status, json, text.trim_end().into())
}

pub(super) fn reason(text: &str) -> Result<Reason> {
    Ok(serde_json::from_value(text.into())?)
}
