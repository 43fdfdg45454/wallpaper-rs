use crate::input::password;
use crate::usage;
use anyhow::{Result, ensure};
use clap::Args;
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use wprs_format::{Charter, Signer};

#[derive(Args, Debug)]
pub struct Policy {
    #[arg(long, help = "Number of root keys that must sign a root")]
    threshold: Option<usize>,
    #[arg(long = "author", value_name = "ID=PUB", help = "Author key; repeat for several keys of an author")]
    authors: Vec<String>,
    #[arg(long, num_args = 0..=1, default_missing_value = "true", value_name = "BOOL")]
    require_author_signature: Option<bool>,
    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL",
        help = "Let authors register online: clients trust per package the key that signed its first version"
    )]
    online_authors: Option<bool>,
    #[arg(long, value_name = "DAYS", help = "Lifetime of the new root [default: 365]")]
    root_days: Option<u32>,
    #[arg(long, value_name = "DAYS", help = "Longest lifetime of a signed index; 14 in a new repository")]
    entry_max_days: Option<u32>,
    #[arg(long, value_name = "BYTES")]
    max_package_size: Option<u64>,
    #[arg(long = "mirror", value_name = "URL", help = "HTTPS mirror; replaces the mirrors")]
    mirrors: Vec<String>,
    #[arg(long, help = "Bits of the index shards; 4 in a new repository")]
    shard_bits: Option<u8>,
}

impl Policy {
    pub fn charter(self) -> Result<Charter> {
        let mut authors = BTreeMap::<String, Vec<String>>::new();
        for author in &self.authors {
            let (id, key) = author.split_once('=').ok_or_else(|| usage(format!("{author} is not ID=PUB")))?;
            authors.entry(id.into()).or_default().push(key.into());
        }
        Ok(Charter {
            authors,
            threshold: self.threshold,
            require_author_signature: self.require_author_signature,
            online_authors: self.online_authors,
            root_days: self.root_days,
            entry_max_days: self.entry_max_days,
            max_package_size: self.max_package_size,
            mirrors: self.mirrors,
            shard_bits: self.shard_bits,
            ..Charter::default()
        })
    }
}

pub fn keygen(name: &str, no_password: bool) -> Result<String> {
    let (secret, public) = (format!("{name}.key"), format!("{name}.pub"));
    ensure!(
        !Path::new(&secret).exists() && !Path::new(&public).exists(),
        "{secret} or {public} already exists"
    );
    let password = (!no_password).then(|| password(true)).transpose()?;
    ensure!(password.as_ref().is_none_or(|p| !p.is_empty()), "the password is empty; use --no-password");
    let pair = wprs_format::keygen(password)?;
    create(&secret, &pair.secret, 0o600)?;
    create(&public, &pair.public, 0o644)?;
    Ok(pair.fingerprint)
}

fn create(path: &str, text: &str, mode: u32) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).mode(mode).open(path)?;
    Ok(file.write_all(text.as_bytes())?)
}

pub fn key_text(key: &str) -> Result<String> {
    Ok(wprs_format::key_text(key)?)
}

pub fn keys(keys: &[String]) -> Result<Vec<String>> {
    keys.iter().map(|key| key_text(key)).collect()
}

pub fn signer(key: &Path) -> Result<Signer> {
    let secret = fs::read_to_string(key)?;
    Signer::new(&secret, None).or_else(|_| Ok(Signer::new(&secret, Some(password(false)?))?))
}
