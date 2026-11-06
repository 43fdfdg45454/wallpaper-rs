use crate::cli::{JSON, Wait};
use crate::daemon::job;
use crate::providers::{add, patient};
use crate::show::{self, table};
use crate::wallhaven_filters::Filters;
use anyhow::Result;
use clap::Subcommand;
use futures_util::TryFutureExt;
use std::collections::HashMap;
use wprs_client::Client;
use wprs_core::model::CollectionsReply;

const SECRET_STDIN: &str = "Read the API key from the first line of standard input instead of asking";
pub const KEEP: &str = "Leave it out to keep the current value or the default; give \"\" to clear it";

#[derive(Subcommand, Debug)]
pub enum Wallhaven {
    #[command(about = "Add a Wallhaven provider with its search options; prints its library source uid", after_long_help = KEEP)]
    Add {
        #[arg(help = "Provider id: a lowercase letter, then lowercase letters, digits, - or _")]
        id: String,
        #[command(flatten)]
        filters: Filters,
        #[arg(long, help = "Add the provider without an API key")]
        no_secret: bool,
        #[arg(long, help = SECRET_STDIN)]
        secret_stdin: bool,
    },
    #[command(about = "Change several search options of a Wallhaven provider at once", after_long_help = KEEP)]
    Set {
        #[arg(help = "Id of the provider")]
        id: String,
        #[command(flatten)]
        filters: Filters,
    },
    #[command(about = "List the collections of a user, or of the account of the API key")]
    Collections {
        #[arg(help = "Id of the provider")]
        id: String,
        #[arg(long, help = "Owner of the collections; the account of the API key when left out")]
        user: Option<String>,
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Pick a new random order for the sorting random and sync")]
    Reseed {
        #[arg(help = "Id of the provider")]
        id: String,
        #[command(flatten)]
        wait: Wait,
    },
}

pub async fn run(client: &Client, command: Wallhaven) -> Result<()> {
    let providers = &client.providers;
    match command {
        Wallhaven::Add { id, filters, no_secret, secret_stdin } => {
            let pairs = filters.pairs()?;
            let options: HashMap<&str, &str> =
                pairs.iter().map(|(key, value)| (*key, value.as_str())).collect();
            add(client, &id, "wallhaven", options, false, (no_secret, secret_stdin)).await?
        }
        Wallhaven::Set { id, filters } => {
            let pairs = filters.pairs()?;
            let changes = pairs.iter().map(|(key, value)| (*key, value.as_str())).collect();
            providers.update(&id, changes).await.map_err(wprs_client::explain)?
        }
        Wallhaven::Collections { id, user, json } => {
            let found = patient(providers.collections(&id, user.as_deref().unwrap_or(""))).await?;
            show::reply(found, json, collections)?
        }
        Wallhaven::Reseed { id, wait } => {
            job(client, providers.reseed(&id).map_ok(|job| ((), job)), &wait).await?
        }
    }
    Ok(())
}

fn collections(reply: CollectionsReply) -> String {
    let rows = reply.collections.into_iter().map(|c| {
        let public = if c.public { "public" } else { "private" };
        vec![c.id.to_string(), c.label, c.count.to_string(), public.to_owned()]
    });
    table(&["ID", "NAME", "PICTURES", "VISIBLE"], rows)
}
