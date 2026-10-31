use crate::account::{approve, join, share, status};
use crate::cli::{JSON, Wait};
use crate::daemon::job;
use crate::input::{confirm, path_arg};
use crate::private::{self, Files};
use crate::providers::patient;
use crate::show;
use anyhow::{Result, bail};
use clap::Subcommand;
use futures_util::TryFutureExt;
use std::io::{self, IsTerminal};
use wprs_client::Client;

const PLAINTEXT: &str =
    "Plain-text sync lets the operator of the server read your library, playlists and settings.";
const RESET: &str = "This deletes every synced record on the server and uploads this computer's library, \
                     playlists and settings under a new key. Every other device has to join again.";

#[derive(Subcommand, Debug)]
pub enum Server {
    #[command(about = "Add the server this computer syncs with, by its https:// address")]
    Add {
        url: String,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Link this computer to an account: prints the code to approve on the server")]
    Link {
        #[arg(
            long,
            default_value = "",
            help = "Name of this computer, seen only by your devices; the host name when empty"
        )]
        name: String,
        #[arg(long, help = "Sync without end-to-end encryption; the server can then read every record")]
        plaintext: bool,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(
        about = "Show the device that waits for a code, then approve it after you confirm",
        long_about = "Show the device that waits for a code, then approve it after you confirm. Approving \
                      asks for the password of the account, read from standard input when it is not a \
                      terminal; an account that signs in through single sign-on approves on the /link page \
                      of the server instead."
    )]
    Approve {
        code: String,
        #[arg(long, help = "Approve without asking; only for a code shown on your own screen")]
        yes: bool,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Show the server, this device, the sync state and the devices of the account")]
    Status {
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Sync now")]
    Sync {
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Stop syncing automatically")]
    Pause,
    #[command(about = "Sync automatically again")]
    Resume,
    #[command(about = "Get the sync key from a device that holds it; with --wait prints the code to compare")]
    Join {
        #[arg(help = "Device that holds the key: its name, its label or its id")]
        device: String,
        #[arg(long, help = "The 6 digits shown by that device, to confirm a running join")]
        code: Option<String>,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Answer a joining device, show the code to compare and send the key once it matches")]
    Share {
        #[arg(help = "The joining device: its name, its label or its id")]
        device: String,
        #[arg(long, help = "The 6 digits shown by the joining device; asked for when missing")]
        code: Option<String>,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(
        about = "Replace the sync key, encrypt every record again and hand the key to the other devices"
    )]
    Rotate {
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Delete the synced data on the server and upload this computer's copy under a new key")]
    Reset {
        #[arg(long, help = "Reset without asking")]
        yes: bool,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Add a repository that another device synced, after checking it, or decline it")]
    Repo {
        #[arg(help = "The repository: its offer id or the first 8 or more digits of its repo id")]
        repo: String,
        #[arg(long, help = "Do not add it on this computer and stop asking")]
        decline: bool,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Sync the secret of a provider with the other devices, end-to-end encrypted")]
    Secret {
        provider: String,
        #[arg(long, help = "Stop syncing it; the copies other devices hold stay")]
        stop: bool,
    },
    #[command(
        about = "Store the sync key on the server, locked with a passphrase, for when no device is left",
        long_about = "Store the sync key on the server, locked with a passphrase, for when no device is \
                      left. After a device is revoked the escrow is stale: set a new, different \
                      passphrase, since the revoked device may know the old one, which is refused."
    )]
    Escrow {
        #[arg(long, help = "Delete the escrow from the server")]
        remove: bool,
        #[arg(long, help = "Delete it without asking")]
        yes: bool,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Get the sync key from the escrow with its passphrase, when no device can share it")]
    Restore {
        #[command(flatten)]
        wait: Wait,
    },
    #[command(subcommand, about = "Your own wallpaper files, end-to-end encrypted on the server")]
    Files(Files),
    #[command(about = "Upload a .wpak package to the repository of the server")]
    Upload {
        file: String,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Forget the server and delete the token and sync key of this computer")]
    Remove {
        #[arg(long, help = "Also delete the local copies of the synced records")]
        wipe: bool,
    },
}

pub async fn run(client: &Client, command: Server) -> Result<()> {
    let sync = &client.sync;
    let shown = |pairs: Vec<(&str, String)>| print!("{}", show::lines(pairs));
    match command {
        Server::Add { url, wait } => job(client, sync.add_server(&url).map_ok(|id| ((), id)), &wait).await?,
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
        Server::Upload { file, wait } => {
            job(client, sync.upload(&path_arg(&file)?).map_ok(|id| ((), id)), &wait).await?
        }
        Server::Remove { wipe } => patient(sync.remove_server(wipe)).await?,
    }
    Ok(())
}
