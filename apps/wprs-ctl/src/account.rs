use crate::cli::Wait;
use crate::daemon::job;
use crate::input::{confirm, secret, typed};
use crate::show::{self, bytes, table, text, when};
use anyhow::{Result, bail};
use futures_util::TryFutureExt;
use std::io::{self, IsTerminal};
use std::time::Duration;
use wprs_client::{Client, explain, parse};
use wprs_core::model::{AccountReply, JobReply, JobState};

pub const NAMES: &str = "Item names on the server are keyed hashes: a device that once held the names key \
                         and has the same file can still tell that the account stores it, even after it is \
                         revoked.";
const OWN: &str = "Approve only a code that your own new device shows on its screen: \
                   someone who sends you a code is trying to get into your account.";

pub fn status(reply: AccountReply) -> String {
    let Some(a) = reply.account else { return "no server is added\n".into() };
    let encryption = match a.encrypted {
        Some(false) => "off: the server can read every record".into(),
        Some(true) => "end to end".into(),
        None => "-".into(),
    };
    let device = a.device.map_or("-".into(), |name| format!("{name} ({})", a.label));
    let storage = format!("{} of {}", bytes(a.used_bytes), bytes(a.quota_bytes));
    let comparison = a.pairing.map_or("-".into(), |p| {
        let name = a.devices.iter().find(|d| d.id == p.device).map_or(p.device.clone(), |d| d.name.clone());
        format!("{name} {}", p.code.unwrap_or("waiting".into()))
    });
    let pairs = [
        ("server", a.url),
        ("name", a.name),
        ("account", a.handle.unwrap_or("-".into())),
        ("device", device),
        ("state", text(&a.state)),
        ("encryption", encryption),
        ("synced", when(a.last_sync)),
        ("records", a.records.to_string()),
        ("storage", storage),
        ("warning", a.warning.unwrap_or("-".into())),
        ("error", a.last_error.unwrap_or("-".into())),
        ("quarantined", a.quarantined.to_string()),
        ("not applied", a.unapplied.to_string()),
        ("comparison", comparison),
        ("recovery", recovery(text(&a.recovery))),
        ("shared secrets", if a.secrets.is_empty() { "-".into() } else { a.secrets.join(", ") }),
        ("private files", format!("{} of {}", bytes(a.files_bytes), bytes(a.files_quota_bytes))),
    ];
    let offers = a.offers.into_iter().map(|o| {
        let fingerprint = o.fingerprints.first().map_or("-", |f| &f[..f.len().min(16)]).to_owned();
        (
            "repository to confirm",
            format!("{} {} {} (fingerprint {fingerprint}, id {})", o.name, o.url, o.repo_id, o.id),
        )
    });
    let rows = a.devices.into_iter().map(|d| {
        let here = if d.current { "this computer" } else { "" };
        vec![d.name, d.label.unwrap_or("-".into()), d.platform, when(d.seen_at), here.into()]
    });
    show::lines(pairs) + &show::lines(offers) + &table(&["DEVICE", "NAME", "PLATFORM", "SEEN", ""], rows)
}

fn recovery(state: String) -> String {
    match state.as_str() {
        "stale" => "stale: a device that may know the passphrase was revoked; set a new, different one \
                    with wprs-ctl server escrow"
            .into(),
        _ => state,
    }
}

pub async fn approve(client: &Client, code: &str, yes: bool, wait: &Wait) -> Result<()> {
    let sync = &client.sync;
    let (_, found) = client.run_job(sync.preview_code(code).map_ok(|id| ((), id)), |_, _| {}).await?;
    let found = found.output.unwrap_or_default();
    let field = |key: &str| found[key].as_str().filter(|t| !t.is_empty()).unwrap_or("not given").to_owned();
    let requested = when(found["requested_at"].as_i64().map(|ms| ms / 1000));
    eprintln!("{OWN}");
    print!(
        "{}",
        show::lines([
            ("platform", field("platform")),
            ("software", field("client")),
            ("requested", requested)
        ])
    );
    if !yes && !confirm("Approve this device?").await? {
        bail!("not approved");
    }
    let password = zeroize::Zeroizing::new(secret("Password of the account: ", false)?);
    job(client, sync.approve_device(code, &password).map_ok(|id| ((), id)), wait).await
}

pub async fn join(client: &Client, device: &str, wait: &Wait) -> Result<()> {
    let id = client.sync.join(device).await.map_err(explain)?;
    if !wait.wait {
        println!("{id}");
        return Ok(());
    }
    let mut shown = false;
    loop {
        let reply: AccountReply = parse(&client.sync.get_status().await.map_err(explain)?)?;
        if let Some(code) = reply.account.and_then(|a| a.pairing?.code).filter(|_| !shown) {
            println!("code = {code}");
            eprintln!("Confirm this code on {device}, and here the code that {device} shows.");
            shown = true;
            match io::stdin().is_terminal() {
                true => {
                    eprint!("Type the code that {device} shows, or nothing to stop: ");
                    let typed = typed().await?;
                    if typed.trim().is_empty() {
                        client.daemon.cancel_job(id).await.map_err(explain)?;
                        bail!("the key was not taken");
                    }
                    client.sync.confirm_share(device, typed.trim()).await.map_err(explain)?;
                }
                false => eprintln!(
                    "Confirm with: wprs-ctl server join {device:?} --code <the code shown on {device}>"
                ),
            }
        }
        let JobReply { job, .. } = parse(&client.daemon.get_job(id).await.map_err(explain)?)?;
        match job.state {
            JobState::Done => {
                println!("{}", if wait.json { serde_json::to_string(&job)? } else { show::summary(&job) });
                return Ok(());
            }
            JobState::Failed | JobState::Cancelled => {
                bail!("{}", job.error.map_or("the join stopped".into(), |e| e.message))
            }
            _ => tokio::time::sleep(Duration::from_millis(300)).await,
        }
    }
}

pub async fn share(client: &Client, device: &str, wait: &Wait) -> Result<()> {
    let sync = &client.sync;
    let (_, compared) = client.run_job(sync.share_key(device).map_ok(|id| ((), id)), |_, _| {}).await?;
    let code = compared.output.as_ref().and_then(|o| o["code"].as_str()).unwrap_or_default().to_owned();
    println!("code = {code}");
    if !io::stdin().is_terminal() {
        eprintln!("Send the key with: wprs-ctl server share {device:?} --code <the code shown on {device}>");
        return Ok(());
    }
    eprint!("Type the code that {device} shows, or nothing to stop: ");
    let typed = typed().await?;
    if typed.trim().is_empty() {
        bail!("the key was not shared");
    }
    job(client, sync.confirm_share(device, typed.trim()).map_ok(|id| ((), id)), wait).await
}
