use crate::cli::{JSON, Wait};
use crate::daemon::job;
use crate::input::{confirm, one, secret, strs};
use crate::show::{self, bytes, table};
use anyhow::{Result, bail};
use clap::Subcommand;
use futures_util::TryFutureExt;
use std::io::{self, IsTerminal};
use std::path::Path;
use wprs_client::{Client, explain};
use wprs_core::model::FilesReply;
use zeroize::Zeroizing;

const STOP: &str = "A file that is on no device any more, for example one of a lost computer, is lost for good \
                    once it leaves the server.";
const ESCROW: &str = "The server will keep the sync key locked with this passphrase, so that a new device can get \
                      it when no device is left. Whoever gets a copy of the server's data can try to guess the \
                      passphrase without limit: use several unrelated words that you use nowhere else, and keep \
                      them somewhere safe.";

#[derive(Subcommand, Debug)]
pub enum Files {
    #[command(about = "List the private files on the server and those waiting to upload")]
    Ls {
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(
        about = "Upload items, or every item of folders, end-to-end encrypted; --stop takes them off the server"
    )]
    Share {
        #[arg(
            required = true,
            help = "Items (path or uid), folders, or with --stop the id of a file on the server"
        )]
        targets: Vec<String>,
        #[arg(long, help = "Stop sharing them and delete them from the server")]
        stop: bool,
        #[arg(long, help = "With --stop, delete without asking")]
        yes: bool,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Download a private file from the server into the library")]
    Fetch {
        file: String,
        #[command(flatten)]
        wait: Wait,
    },
}

fn listing(reply: FilesReply) -> String {
    let folders = reply.folders.into_iter().map(|f| ("folder", f));
    let rows = reply.files.into_iter().map(|f| {
        let state = match (f.stored, f.shared) {
            (false, _) => "waiting",
            (true, true) => "shared from here",
            (true, false) => "on the server",
        };
        let id = if f.id.is_empty() { "-".into() } else { f.id };
        vec![id, f.name, bytes(f.size), f.item.unwrap_or("-".into()), state.into()]
    });
    show::lines(folders) + &table(&["FILE", "NAME", "SIZE", "ITEM", "STATE"], rows)
}

async fn target(client: &Client, text: &str) -> Result<String> {
    let hex = text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    match () {
        () if Path::new(text).is_dir() => Ok(std::path::absolute(text)?.display().to_string()),
        () if hex => Ok(text.to_owned()),
        () => one(client, text).await,
    }
}

pub async fn run(client: &Client, command: Files) -> Result<()> {
    let sync = &client.sync;
    match command {
        Files::Ls { json } => show::reply(sync.list_files().await.map_err(explain)?, json, listing)?,
        Files::Share { targets, stop, yes, wait } => {
            let mut resolved = Vec::new();
            for text in &targets {
                resolved.push(target(client, text).await?);
            }
            if stop {
                eprintln!("{STOP}");
                if !yes && !confirm("Delete these files from the server?").await? {
                    bail!("nothing was deleted");
                }
            }
            job(client, sync.share_files(&strs(&resolved), !stop).map_ok(|id| ((), id)), &wait).await?
        }
        Files::Fetch { file, wait } => {
            job(client, sync.fetch_file(&file).map_ok(|id| ((), id)), &wait).await?
        }
    }
    Ok(())
}

pub async fn repo(client: &Client, repo: &str, decline: bool, wait: &Wait) -> Result<()> {
    let id = client.sync.confirm_repo(repo, !decline).await.map_err(explain)?;
    if decline {
        println!("declined; other devices keep it");
        return Ok(());
    }
    job(client, async { Ok(((), id)) }, wait).await
}

pub async fn escrow(client: &Client, remove: bool, yes: bool, wait: &Wait) -> Result<()> {
    let passphrase = match remove {
        true => {
            eprintln!("Without the escrow, only a device that holds the sync key can give it to a new one.");
            if !yes && !confirm("Delete the escrow from the server?").await? {
                bail!("the escrow was kept");
            }
            Zeroizing::new(String::new())
        }
        false => {
            eprintln!("{ESCROW}");
            let first = Zeroizing::new(secret("Recovery passphrase: ", false)?);
            let again =
                io::stdin().is_terminal().then(|| secret("Repeat the passphrase: ", false)).transpose()?;
            if again.is_some_and(|again| *again != *first) {
                bail!("the passphrases differ");
            }
            first
        }
    };
    job(client, client.sync.set_escrow(&passphrase).map_ok(|id| ((), id)), wait).await
}

pub async fn restore(client: &Client, wait: &Wait) -> Result<()> {
    let passphrase = Zeroizing::new(secret("Recovery passphrase: ", false)?);
    job(client, client.sync.restore(&passphrase).map_ok(|id| ((), id)), wait).await
}
