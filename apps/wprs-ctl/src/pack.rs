use crate::keys::{key_text, signer};
use anyhow::{Result, ensure};
use clap::Subcommand;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use wprs_format::{
    Limits, MANIFEST, Manifest, build, fingerprint, open, public_key, sha256_file, sign_source,
    verify_package,
};

#[derive(Subcommand, Debug)]
pub enum Pack {
    #[command(about = "Build a .wpak package from a folder that holds manifest.json")]
    Build {
        dir: PathBuf,
        #[arg(short, long, help = "Package file; <id>-<version>.wpak in the current folder when omitted")]
        out: Option<PathBuf>,
        #[arg(long, help = "Print the result as JSON")]
        json: bool,
    },
    #[command(about = "Sign the manifest of a package folder; run it before pack build")]
    Sign {
        dir: PathBuf,
        #[arg(long, help = "Secret key file of the author")]
        key: PathBuf,
    },
    #[command(about = "Check packages and, with --key, their author signature")]
    Validate {
        #[arg(required = true, value_name = "FILE")]
        files: Vec<PathBuf>,
        #[arg(long = "key", value_name = "PUB", help = "Public key file or text of an accepted author")]
        keys: Vec<String>,
        #[arg(long, help = "Print one JSON object per file")]
        json: bool,
    },
}

pub fn run(command: Pack) -> Result<()> {
    match command {
        Pack::Build { dir, out, json } => {
            let out = match out {
                Some(out) => out,
                None => {
                    let manifest = Manifest::from_json(&fs::read(dir.join(MANIFEST))?)?;
                    format!("{}-{}.wpak", manifest.id, manifest.version).into()
                }
            };
            let (sha256, size) = (build(&dir, &out, &Limits::default())?, fs::metadata(&out)?.len());
            match json {
                true => println!("{}", json!({"path": out, "sha256": sha256, "size": size})),
                false => println!("{sha256} {size} {}", out.display()),
            }
        }
        Pack::Sign { dir, key } => println!("{}", sign_source(&dir, &signer(&key)?)?),
        Pack::Validate { files, keys, json } => {
            let keys = keys.iter().map(|k| key_text(k)).collect::<Result<Vec<_>>>()?;
            let keys =
                keys.iter().map(|k| Ok((fingerprint(k)?, public_key(k)?))).collect::<Result<Vec<_>>>()?;
            let pairs: Vec<(&str, &str)> = keys.iter().map(|(fp, key)| (fp.as_str(), key.as_str())).collect();
            let reports: Vec<(bool, String)> = files.iter().map(|file| report(file, &pairs, json)).collect();
            reports.iter().for_each(|(_, line)| println!("{line}"));
            let invalid = reports.iter().filter(|(valid, _)| !valid).count();
            ensure!(invalid == 0, "{invalid} of {} packages are invalid", files.len());
        }
    }
    Ok(())
}

fn report(file: &Path, keys: &[(&str, &str)], json: bool) -> (bool, String) {
    let (checked, sha256) = (check(file, keys), sha256_file(file).ok());
    let line = match (&checked, json) {
        (Ok((m, signer)), false) => {
            let (sha256, signer) = (sha256.unwrap_or_default(), signer.as_deref().unwrap_or("-"));
            format!("ok {} {} {} sha256={sha256} signer={signer}", m.kind, m.id, m.version)
        }
        (Err(e), false) => format!("{}: {e}", file.display()),
        (_, true) => {
            let (size, valid) = (fs::metadata(file).ok().map(|m| m.len()), checked.is_ok());
            let (manifest, signer, error) = match &checked {
                Ok((m, signer)) => {
                    (json!({"id": m.id, "version": m.version, "kind": m.kind}), json!(signer), json!(null))
                }
                Err(e) => (json!(null), json!(null), json!(e.to_string())),
            };
            json!({"file": file, "valid": valid, "sha256": sha256, "size": size, "manifest": manifest,
                "signer": signer, "error": error})
            .to_string()
        }
    };
    (checked.is_ok(), line)
}

fn check(file: &Path, keys: &[(&str, &str)]) -> Result<(Manifest, Option<String>)> {
    let mut package = open(file, &Limits::default())?;
    let signer = (!keys.is_empty()).then(|| verify_package(&mut package, keys)).transpose()?;
    Ok((package.manifest, signer))
}
