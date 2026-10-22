use crate::usage;
use anyhow::{Result, bail};
use serde_json::json;
use std::io::{self, BufRead, IsTerminal, Write};
use wprs_depth::{Asset, Places};

pub fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1e6)
}

pub fn consent(asset: &Asset, yes: bool) -> Result<()> {
    if !wprs_depth::allowed() {
        let hint = format!("use wprs-ctl models install {} --file", asset.id);
        bail!("downloads are turned off by models.allow_download or a broken config; {hint}");
    }
    let (size, hosts) = (megabytes(asset.bytes), asset.hosts());
    eprintln!("{}: {size} from {hosts}, license {} ({})", asset.title, asset.license, asset.license_url);
    if yes {
        return Ok(());
    }
    if !io::stdin().is_terminal() {
        return Err(usage("add --yes to accept the download"));
    }
    eprint!("Download it now? [y/N] ");
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    match answer.trim() {
        "y" | "Y" | "yes" => Ok(()),
        _ => bail!("nothing was downloaded"),
    }
}

pub fn progress() -> impl FnMut(u64, u64) -> bool {
    let (terminal, mut shown) = (io::stderr().is_terminal(), u64::MAX);
    move |done, total| {
        let percent = done * 100 / total.max(1);
        if percent != shown && (terminal || percent % 10 == 0) {
            shown = percent;
            let line = format!("{percent:>3}% {} of {}", megabytes(done), megabytes(total));
            if terminal {
                eprint!("\r{line}")
            } else {
                eprintln!("{line}")
            }
            io::stderr().flush().ok();
        }
        true
    }
}

pub fn list(places: &Places, json: bool) -> Result<()> {
    let rows: Vec<_> =
        wprs_depth::models::assets().into_iter().map(|a| (a, places.installed(&a).is_some())).collect();
    if json {
        let list = rows.iter().map(|(a, on)| {
            json!({"id": a.id, "title": a.title, "bytes": a.bytes, "sha256": a.sha256, "license": a.license,
                "license_url": a.license_url, "urls": a.urls, "installed": on})
        });
        println!("{}", serde_json::to_string_pretty(&list.collect::<Vec<_>>())?);
        return Ok(());
    }
    for (a, on) in rows {
        let state = if on { "installed" } else { "not installed" };
        println!("{:<24} {:>9}  {:<10} {:<11} {state}", a.id, megabytes(a.bytes), a.license, a.hosts());
    }
    Ok(())
}
