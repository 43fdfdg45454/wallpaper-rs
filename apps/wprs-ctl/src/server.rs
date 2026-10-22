use crate::account::status;
use crate::cli::{JSON, Wait};
use crate::daemon::job;
use crate::input::{confirm, path_arg};
use crate::providers::patient;
use crate::show;
use anyhow::{Result, bail};
use clap::Subcommand;
use futures_util::TryFutureExt;
use std::io::{self, IsTerminal};
use wprs_client::Client;

const PLAINTEXT: &str =
    "Plain-text sync lets the operator of the server read your library, playlists and settings.";

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
    #[command(about = "Approve the code shown by a new device of your account")]
    Approve {
        code: String,
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
    #[command(
        about = "Get the sync key of an encrypted account from one of its devices; prints the code to compare"
    )]
    Join {
        #[arg(help = "Device that holds the key: its name, its label or its id")]
        device: String,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Send the sync key to a joining device once its code matches")]
    Share {
        device: String,
        #[arg(help = "The 12 digits shown by the joining device")]
        code: String,
        #[command(flatten)]
        wait: Wait,
    },
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
                if io::stdin().is_terminal() && !confirm("Link without end-to-end encryption?")? {
                    bail!("not linked");
                }
            }
            let start = sync.link(&name, plaintext).map_ok(|(code, url, id)| {
                shown(vec![("code", code), ("url", url)]);
                ((), id)
            });
            job(client, start, &wait).await?
        }
        Server::Approve { code, wait } => {
            job(client, sync.approve(&code).map_ok(|id| ((), id)), &wait).await?
        }
        Server::Status { json } => show::reply(sync.get_status().await?, json, status)?,
        Server::Sync { wait } => job(client, sync.sync_now().map_ok(|id| ((), id)), &wait).await?,
        Server::Pause => sync.set_paused(true).await?,
        Server::Resume => sync.set_paused(false).await?,
        Server::Join { device, wait } => {
            let start = sync.join(&device).map_ok(|(code, id)| {
                shown(vec![("code", code)]);
                ((), id)
            });
            job(client, start, &wait).await?
        }
        Server::Share { device, code, wait } => {
            job(client, sync.share_key(&device, &code).map_ok(|id| ((), id)), &wait).await?
        }
        Server::Upload { file, wait } => {
            job(client, sync.upload(&path_arg(&file)?).map_ok(|id| ((), id)), &wait).await?
        }
        Server::Remove { wipe } => patient(sync.remove_server(wipe)).await?,
    }
    Ok(())
}
