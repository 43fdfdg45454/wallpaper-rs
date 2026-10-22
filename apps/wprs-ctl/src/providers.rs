use crate::Usage;
use crate::cli::{JSON, Wait};
use crate::daemon::job;
use crate::input::{pairs, secret};
use crate::show::{self, bytes, list, table, text, when};
use anyhow::{Context, Result, ensure};
use clap::Subcommand;
use futures_util::TryFutureExt;
use std::time::Duration;
use wprs_client::{Client, explain, parse};
use wprs_core::model::{ProvidersReply, Schema, SecretField};

const SECRET_STDIN: &str = "Read the secret from the first line of standard input instead of asking";
const PATIENCE: Duration = Duration::from_secs(120);
const SECRETS: [&str; 4] = ["password", "api_key", "token", "secret"];

#[derive(Subcommand, Debug)]
pub enum Provider {
    #[command(about = "List the providers")]
    Ls {
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Add a provider, asking for its password or API key; prints its library source uid")]
    Add {
        #[arg(help = "Provider id: a lowercase letter, then lowercase letters, digits, - or _")]
        id: String,
        #[arg(help = "bing, apod, wikimedia, wallhaven, webdav, nextcloud or immich")]
        kind: String,
        #[arg(value_name = "KEY=VALUE", help = "Options of the kind; see wprs-ctl schema")]
        options: Vec<String>,
        #[arg(long, help = "Allow plain http to a remote server: the secret then travels unencrypted")]
        allow_plain_http: bool,
        #[arg(long, help = "Add the provider without a secret")]
        no_secret: bool,
        #[arg(long, help = SECRET_STDIN)]
        secret_stdin: bool,
    },
    #[command(about = "Change one option of a provider")]
    Set { id: String, key: String, value: String },
    #[command(about = "Store the password or API key of a provider in the keyring")]
    Secret {
        id: String,
        #[arg(long, help = SECRET_STDIN)]
        secret_stdin: bool,
    },
    #[command(about = "Delete the stored password or API key of a provider")]
    ForgetSecret { id: String },
    #[command(about = "Download new wallpapers of a provider, or of every provider when omitted")]
    Sync {
        id: Option<String>,
        #[command(flatten)]
        wait: Wait,
    },
    #[command(about = "Remove a provider and its items")]
    Rm {
        id: String,
        #[arg(long, help = "Also delete the downloaded files")]
        purge: bool,
    },
}

pub async fn run(client: &Client, command: Provider) -> Result<()> {
    let providers = &client.providers;
    match command {
        Provider::Ls { json } => show::reply(providers.list().await?, json, list_providers)?,
        Provider::Add { id, kind, options, allow_plain_http, no_secret, secret_stdin } => {
            let schema: Schema = parse(&client.daemon.get_schema().await?)?;
            let kinds = || list(schema.providers.iter().map(|p| &p.kind));
            let known = schema.providers.iter().find(|p| p.kind == kind);
            let known =
                known.with_context(|| format!("unknown provider kind {kind}; the kinds are {}", kinds()))?;
            let mut options = pairs(&options)?;
            options.keys().try_for_each(|key| public(key))?;
            if allow_plain_http {
                options.insert("allow_plain_http", "true");
            }
            let secret =
                known.secret.as_ref().filter(|_| !no_secret).map(|f| ask(f, &id, &kind, secret_stdin));
            let secret = secret.transpose()?.unwrap_or_default();
            println!("{}", patient(providers.add(&id, &kind, options, &secret)).await?)
        }
        Provider::Set { id, key, value } => {
            public(&key)?;
            let had = has_secret(client, &id).await?;
            providers.set(&id, &key, &value).await?;
            if had && !has_secret(client, &id).await? {
                eprintln!(
                    "the secret of {id} was deleted; store a new one with wprs-ctl provider secret {id}"
                );
            }
        }
        Provider::Secret { id, secret_stdin } => {
            let secret = secret(&format!("secret of {id}: "), secret_stdin)?;
            ensure!(!secret.is_empty(), "the secret is empty");
            patient(providers.set_secret(&id, &secret)).await?
        }
        Provider::ForgetSecret { id } => providers.forget_secret(&id).await?,
        Provider::Sync { id, wait } => {
            job(client, providers.sync(id.as_deref().unwrap_or("")).map_ok(|id| ((), id)), &wait).await?
        }
        Provider::Rm { id, purge } => providers.remove(&id, purge).await?,
    }
    Ok(())
}

fn public(key: &str) -> Result<()> {
    ensure!(
        !SECRETS.contains(&key),
        Usage(format!("{key} is a secret; it is asked for, never given as an argument"))
    );
    Ok(())
}

fn ask(field: &SecretField, id: &str, kind: &str, stdin: bool) -> Result<String> {
    let optional = if field.required { "" } else { " (empty for none)" };
    let secret = secret(&format!("{} of {id}{optional}: ", field.name.replace('_', " ")), stdin)?;
    let hint = format!("{kind} needs its {}; use --no-secret to add it without one", field.name);
    ensure!(!field.required || !secret.is_empty(), hint);
    Ok(secret)
}

pub async fn patient<T>(call: impl Future<Output = zbus::Result<T>>) -> Result<T> {
    let answer = tokio::time::timeout(PATIENCE, call).await;
    answer.context("wprsd did not answer within 120 s")?.map_err(explain)
}

async fn has_secret(client: &Client, id: &str) -> Result<bool> {
    let reply: ProvidersReply = parse(&client.providers.list().await?)?;
    Ok(reply.providers.iter().any(|p| p.id == id && p.has_secret))
}

pub fn list_providers(reply: ProvidersReply) -> String {
    let rows = reply.providers.into_iter().map(|p| {
        let (state, items, cache) = (text(&p.state), p.items.to_string(), bytes(p.cache_bytes));
        let error = p.last_error.unwrap_or_default();
        vec![p.id, p.kind, state, items, cache, when(p.last_sync), when(p.next_sync), error]
    });
    table(&["PROVIDER", "KIND", "STATE", "ITEMS", "CACHE", "SYNCED", "NEXT", "ERROR"], rows)
}
