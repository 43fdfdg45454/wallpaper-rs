use crate::cli::JSON;
use crate::input::secret;
use crate::show::{table, when};
use anyhow::Result;
use clap::{Args, Subcommand};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::Method;
use serde_json::json;
use wprs_sync::Api;

#[derive(Args, Debug)]
pub struct Moderate {
    #[arg(long, help = "Address of the server, such as https://wallpapers.example.org")]
    server: String,
    #[command(subcommand)]
    action: Action,
}

#[derive(Subcommand, Debug)]
pub enum Action {
    #[command(
        about = "Take a package out of the catalog and search; whoever installed it keeps it and its updates"
    )]
    Hide {
        id: String,
        #[arg(long, help = "Reason the author sees, at least 10 characters")]
        reason: String,
    },
    #[command(about = "Show a hidden package in the catalog and search again")]
    Unhide { id: String },
    #[command(about = "List the hidden packages and why")]
    Hidden {
        #[arg(long, help = JSON)]
        json: bool,
    },
}

pub async fn run(m: Moderate) -> Result<()> {
    let token = match std::env::var("WPRS_SERVER_TOKEN") {
        Ok(token) => token,
        Err(_) => secret("Personal token of a moderator with the mod scope: ", false)?,
    };
    let api = Api::new(&m.server)?.with_token(token.trim());
    let path = |id: &str| format!("/api/v1/mod/hidden/{}", utf8_percent_encode(id, NON_ALPHANUMERIC));
    match m.action {
        Action::Hide { id, reason } => {
            drop(api.post("/api/v1/mod/hidden", json!({"id": id, "reason": reason})).await?)
        }
        Action::Unhide { id } => drop(api.call(Method::DELETE, &path(&id), None).await?),
        Action::Hidden { json: true } => println!("{}", api.get("/api/v1/mod/hidden").await?),
        Action::Hidden { json: false } => {
            let list = api.get("/api/v1/mod/hidden").await?;
            let text = |v: &serde_json::Value, k: &str| v[k].as_str().unwrap_or_default().to_owned();
            let rows = list["items"].as_array().into_iter().flatten().map(|h| {
                vec![text(h, "id"), when(h["hidden_at"].as_i64().map(|ms| ms / 1000)), text(h, "reason")]
            });
            print!("{}", table(&["PACKAGE", "HIDDEN", "REASON"], rows));
        }
    }
    Ok(())
}
