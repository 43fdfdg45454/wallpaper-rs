use crate::cli::{JSON, Page, Wait};
use crate::daemon::job;
use crate::input::confirm;
use crate::items::range;
use crate::show::{self, bytes, list, prefix, short, table, text};
use anyhow::{Context, Result, bail, ensure};
use clap::Subcommand;
use futures_util::{StreamExt, TryFutureExt};
use std::collections::{BTreeSet, HashMap};
use std::time::Duration;
use wprs_client::{Client, error_name, parse};
use wprs_core::dbus::error::STALE;
use wprs_core::model::{BrowsePage, InstalledReply};

const SPEC: &str = "[REPO/]ID[@VERSION]: a package id, optionally with a repository id prefix and a version";

#[derive(Subcommand, Debug)]
pub enum Pkg {
    #[command(about = "Search the packages of the added repositories")]
    Search {
        #[arg(default_value = "")]
        query: String,
        #[command(flatten)]
        page: Page,
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Show the previews of a mature package without blur from now on")]
    Reveal {
        #[arg(help = SPEC)]
        spec: String,
    },
    #[command(about = "Print the path of the preview image of a package")]
    Preview {
        #[arg(help = SPEC)]
        spec: String,
    },
    #[command(about = "Install a package, or update it to its newest version")]
    Install {
        #[arg(help = SPEC)]
        spec: String,
        #[arg(long, help = "Install even when the signed index of the repository expired")]
        allow_stale: bool,
        #[arg(long, help = "Allow a version older than the installed one")]
        downgrade: bool,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Install every available update")]
    Upgrade {
        #[arg(long, help = "Install even when the signed index of a repository expired")]
        allow_stale: bool,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Uninstall every version of a package, or one version")]
    Uninstall {
        id: String,
        #[arg(long, default_value = "")]
        version: String,
    },
    #[command(about = "List the installed packages")]
    Ls {
        #[arg(long, help = JSON)]
        json: bool,
    },
}

pub async fn run(client: &Client, command: Pkg) -> Result<()> {
    let repos = &client.repos;
    match command {
        Pkg::Search { query, page, json } => {
            show::reply(repos.browse(&query, page.offset, page.limit).await?, json, browse)?
        }
        Pkg::Reveal { spec } => repos.reveal(&spec).await?,
        Pkg::Preview { spec } => {
            let path = tokio::time::timeout(Duration::from_secs(60), preview(client, &spec)).await;
            let path = path.context("no preview arrived within 60 s")??;
            ensure!(!path.is_empty(), "no preview");
            println!("{path}")
        }
        Pkg::Install { spec, allow_stale, downgrade, wait } => {
            install(client, &spec, allow_stale, downgrade, &wait, &mut HashMap::new()).await?
        }
        Pkg::Upgrade { allow_stale, wait } => {
            let installed: InstalledReply = parse(&repos.list_installed().await?)?;
            let updates = installed.packages.iter().filter(|p| p.update.is_some());
            let (mut asked, mut declined) = (HashMap::new(), None);
            for spec in updates.map(|p| format!("{}/{}", p.repo_id, p.id)).collect::<BTreeSet<_>>() {
                match install(client, &spec, allow_stale, false, &wait, &mut asked).await {
                    Err(e) if error_name(&e) == Some(STALE) => declined = declined.or(Some(e)),
                    done => done?,
                }
            }
            declined.map_or(Ok(()), Err)?
        }
        Pkg::Uninstall { id, version } => repos.uninstall(&id, &version).await?,
        Pkg::Ls { json } => show::reply(repos.list_installed().await?, json, installed)?,
    }
    Ok(())
}

async fn preview(client: &Client, spec: &str) -> Result<String> {
    let mut ready = client.repos.receive_preview_ready().await?;
    if let Some(path) = client.repos.request_previews(&[spec]).await?.remove(spec) {
        return Ok(path);
    }
    while let Some(signal) = ready.next().await {
        if let Ok(args) = signal.args()
            && args.spec == spec
        {
            return Ok(args.path.into());
        }
    }
    bail!("the connection to the session bus was closed")
}

async fn install(
    client: &Client,
    spec: &str,
    allow_stale: bool,
    downgrade: bool,
    wait: &Wait,
    asked: &mut HashMap<String, bool>,
) -> Result<()> {
    let start = |stale| client.repos.install(spec, stale, downgrade).map_ok(|id| ((), id));
    match job(client, start(allow_stale), wait).await {
        Err(e) if !allow_stale && error_name(&e) == Some(STALE) => {
            let (repo, name) = offered_by(client, spec).await?;
            let question = format!("Repository {name} is stale (its signed index expired). Install anyway?");
            let yes = match asked.get(&repo) {
                Some(yes) => *yes,
                None => confirm(&question).await?,
            };
            asked.insert(repo, yes);
            if yes { job(client, start(true), wait).await } else { Err(e) }
        }
        done => done,
    }
}

async fn offered_by(client: &Client, spec: &str) -> Result<(String, String)> {
    let (repo, rest) = spec.split_once('/').unwrap_or(("", spec));
    let id = rest.split('@').next().unwrap_or(rest);
    let page: BrowsePage = parse(&client.repos.browse(id, 0, 500).await?)?;
    let found = page.packages.into_iter().find(|p| p.id == id && p.repo_id.starts_with(repo));
    Ok(found.map_or_else(|| (repo.into(), format!("of {id}")), |p| (p.repo_id, p.repo_name)))
}

pub fn browse(page: BrowsePage) -> String {
    let count = range(page.offset, page.packages.len(), page.total);
    let rows = page.packages.into_iter().map(|p| {
        let installed = list(&p.installed) + if p.update { " (update available)" } else { "" };
        let flashing = (p.photosensitive == Some(true)).then_some("photosensitive");
        let shown = (p.revealed && p.content.as_deref() == Some("mature")).then_some("shown");
        let content = list([p.content.as_deref().or(Some("unrated")), flashing, shown].into_iter().flatten());
        vec![prefix(&p.repo_id), p.id, p.version, p.kind, content, installed, p.title]
    });
    let head = ["REPO", "ID", "VERSION", "KIND", "CONTENT", "INSTALLED", "TITLE"];
    show::lines([("query", page.query), ("packages", count)]) + &table(&head, rows)
}

pub fn installed(reply: InstalledReply) -> String {
    let notes: String = reply
        .packages
        .iter()
        .filter(|p| p.update.is_some() && !p.changes.is_empty())
        .map(|p| {
            let new = p.update.as_deref().unwrap_or_default();
            format!("\nWhat's new in {} {new}:\n{}\n", p.id, wprs_core::markup::plain(&p.changes))
        })
        .collect();
    let rows = reply.packages.into_iter().map(|p| {
        let (item, size) = (p.item.as_deref().map_or("-".into(), short), bytes(p.size));
        let (update, revoked) = (p.update.unwrap_or("-".into()), p.revoked.map_or("-".into(), |r| text(&r)));
        vec![prefix(&p.repo_id), p.id, p.version, size, item, update, revoked]
    });
    table(&["REPO", "ID", "VERSION", "SIZE", "ITEM", "UPDATE", "REVOKED"], rows) + &notes
}
