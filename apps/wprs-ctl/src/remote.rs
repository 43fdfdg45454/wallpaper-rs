use crate::cli::{JSON, Wait};
use crate::daemon::job;
use crate::show::{self, prefix, table, text, time};
use anyhow::Result;
use clap::Subcommand;
use futures_util::TryFutureExt;
use wprs_client::Client;
use wprs_core::model::ReposReply;

#[derive(Subcommand, Debug)]
pub enum Remote {
    #[command(about = "List the repositories added to this computer")]
    Ls {
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Add a repository by a URL that carries fingerprint= or repo_id=")]
    Add {
        url: String,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(
        about = "Check a repository, or every repository when omitted, for updates and revocations",
        long_about = "Check a repository, or every repository when omitted, for updates and revocations. \
            A broken repository, whose stored copy cannot be read, is fetched again from its address; \
            when that address is lost, add it again with its URL or remove it."
    )]
    Refresh {
        #[arg(help = "Repository id, or at least its first 8 hex digits")]
        repo: Option<String>,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Remove a repository")]
    Rm {
        #[arg(help = "Repository id, or at least its first 8 hex digits")]
        repo: String,
        #[arg(long, help = "Also uninstall its packages")]
        uninstall: bool,
    },
}

pub async fn run(client: &Client, command: Remote) -> Result<()> {
    let repos = &client.repos;
    match command {
        Remote::Ls { json } => show::reply(repos.list().await?, json, list)?,
        Remote::Add { url, wait } => job(client, repos.add(&url).map_ok(|id| ((), id)), &wait).await?,
        Remote::Refresh { repo, wait } => {
            job(client, repos.refresh(repo.as_deref().unwrap_or("")).map_ok(|id| ((), id)), &wait).await?
        }
        Remote::Rm { repo, uninstall } => repos.remove(&repo, uninstall).await?,
    }
    Ok(())
}

pub fn list(reply: ReposReply) -> String {
    let rows = reply.repos.into_iter().map(|r| {
        let broken = r.broken.map(|why| format!("broken, refresh or remove it: {why}"));
        let status = broken.into_iter().chain(r.warnings.iter().map(text)).chain(r.last_error);
        let status = status.collect::<Vec<_>>().join(", ");
        let (packages, checked, expires) = (r.packages.to_string(), time(r.checked), time(r.entry_expires));
        vec![prefix(&r.repo_id), r.name, packages, checked, expires, status]
    });
    table(&["REPO", "NAME", "PACKAGES", "CHECKED", "EXPIRES", "STATUS"], rows)
}
