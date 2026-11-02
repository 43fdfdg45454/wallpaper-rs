use crate::account::{approve, join, share, status};
use crate::daemon::job;
use crate::input::{confirm, path_arg};
use crate::private;
use crate::providers::patient;
use crate::server::Server;
use crate::show;
use anyhow::{Result, bail};
use futures_util::TryFutureExt;
use std::io::{self, IsTerminal};
use wprs_client::Client;

const PLAINTEXT: &str =
    "Plain-text sync lets the operator of the server read your library, playlists and settings.";
const RESET: &str = "This deletes every synced record on the server and uploads this computer's library, \
                     playlists and settings under a new key. Every other device has to join again.";

pub async fn run(client: &Client, command: Server) -> Result<()> {
    let sync = &client.sync;
    let shown = |pairs: Vec<(&str, String)>| print!("{}", show::lines(pairs));
    match command {
        Server::Add { url, fingerprint: None, wait } => {
            job(client, sync.add_server(&url).map_ok(|id| ((), id)), &wait).await?
        }
        Server::Add { url, fingerprint: Some(pin), wait } => {
            job(client, sync.add_pinned_server(&url, &pin).map_ok(|id| ((), id)), &wait).await?
        }
        Server::Link { name, plaintext, wait } => {
            if plaintext {
                eprintln!("{PLAINTEXT}");
                if io::stdin().is_terminal() && !confirm("Link without end-to-end encryption?").await? {
                    bail!("not linked");
                }
            }
            let start = sync.link(&name, plaintext).map_ok(|(code, url, id)| {
                shown(vec![("code", code), ("url", url)]);
                ((), id)
            });
            job(client, start, &wait).await?
        }
        Server::Approve { code, yes, wait } => approve(client, &code, yes, &wait).await?,
        Server::Status { json } => show::reply(sync.get_status().await?, json, status)?,
        Server::Sync { wait } => job(client, sync.sync_now().map_ok(|id| ((), id)), &wait).await?,
        Server::Pause => sync.set_paused(true).await?,
        Server::Resume => sync.set_paused(false).await?,
        Server::Join { device, code: Some(code), wait } => {
            job(client, sync.confirm_share(&device, &code).map_ok(|id| ((), id)), &wait).await?
        }
        Server::Join { device, code: None, wait } => join(client, &device, &wait).await?,
        Server::Share { device, code: Some(code), wait } => {
            job(client, sync.confirm_share(&device, &code).map_ok(|id| ((), id)), &wait).await?
        }
        Server::Share { device, code: None, wait } => share(client, &device, &wait).await?,
        Server::Rotate { wait } => job(client, sync.rotate().map_ok(|id| ((), id)), &wait).await?,
        Server::Reset { yes, wait } => {
            eprintln!("{RESET}");
            if !yes && !confirm("Reset the synced data of the account?").await? {
                bail!("nothing was reset");
            }
            job(client, sync.reset().map_ok(|id| ((), id)), &wait).await?
        }
        Server::Repo { repo, decline, wait } => private::repo(client, &repo, decline, &wait).await?,
        Server::Secret { provider, stop } => sync.share_secret(&provider, !stop).await?,
        Server::Escrow { remove, yes, wait } => private::escrow(client, remove, yes, &wait).await?,
        Server::Restore { wait } => private::restore(client, &wait).await?,
        Server::Files(files) => private::run(client, files).await?,
        Server::Upload { file, unlisted, wait } => {
            let visibility = if unlisted { "unlisted" } else { "public" };
            job(client, sync.upload_as(&path_arg(&file)?, visibility).map_ok(|id| ((), id)), &wait).await?
        }
        Server::Packages { json } => {
            let found: serde_json::Value = serde_json::from_str(&patient(sync.list_packages()).await?)?;
            if json {
                println!("{found}");
            }
            for v in found["items"].as_array().into_iter().flatten().filter(|_| !json) {
                let text = |k: &str| v[k].as_str().unwrap_or("-").to_owned();
                println!("{} {} {} {}", text("package"), text("version"), text("state"), text("visibility"));
            }
        }
        Server::Visibility { id, visibility, version } => {
            patient(sync.set_visibility(&id, version.as_deref().unwrap_or(""), &visibility)).await?
        }
        Server::Remove { wipe } => patient(sync.remove_server(wipe)).await?,
    }
    Ok(())
}
