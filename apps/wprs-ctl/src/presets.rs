use crate::cli::JSON;
use crate::input::{one, pairs};
use crate::profiles::named;
use crate::show::{self, short, table};
use anyhow::{Context, Result};
use clap::Subcommand;
use wprs_client::{Client, parse};
use wprs_core::model::PresetsReply;

#[derive(Subcommand, Debug)]
pub enum Preset {
    #[command(about = "List the presets of an item, or the global presets")]
    Ls {
        #[arg(value_name = "ITEM", help = "Item whose presets to list; global presets when omitted")]
        item: Option<String>,
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Save a preset and print its uid")]
    Save {
        #[arg(value_name = "ITEM", help = "Item of the preset, or - for a global preset")]
        item: String,
        #[arg(help = "Name of the new preset")]
        name: String,
        #[arg(required = true, value_name = "KEY=VALUE", help = "fit=<placement> or background=#rrggbb")]
        values: Vec<String>,
    },
    #[command(about = "Delete a preset")]
    Rm {
        #[arg(
            value_name = "PRESET",
            allow_hyphen_values = true,
            help = "Preset uid, ITEM:NAME, or -:NAME for a global preset"
        )]
        preset: String,
    },
}

pub async fn preset(client: &Client, command: Preset) -> Result<()> {
    let profiles = &client.profiles;
    match command {
        Preset::Ls { item, json } => {
            let item = scope(client, item.as_deref().unwrap_or("-")).await?;
            show::reply(profiles.list_presets(&item).await?, json, presets)?
        }
        Preset::Save { item, name, values } => {
            let item = scope(client, &item).await?;
            println!("{}", profiles.save_preset(&item, &name, pairs(&values)?).await?)
        }
        Preset::Rm { preset } => {
            let uid = match named(&preset) {
                Some((item, name)) => {
                    let item = scope(client, item).await?;
                    let reply: PresetsReply = parse(&profiles.list_presets(&item).await?)?;
                    let mut found =
                        reply.presets.into_iter().filter(|p| p.item.as_deref().unwrap_or("") == item);
                    let found = found.find(|p| p.name.eq_ignore_ascii_case(name));
                    found.map(|p| p.uid).with_context(|| format!("no preset named {name}"))?
                }
                None => preset,
            };
            profiles.delete_preset(&uid).await?
        }
    }
    Ok(())
}

async fn scope(client: &Client, item: &str) -> Result<String> {
    if item == "-" { Ok(String::new()) } else { one(client, item).await }
}

pub fn presets(reply: PresetsReply) -> String {
    let rows = reply.presets.into_iter().map(|p| {
        let (item, fit) =
            (p.item.as_deref().map_or("-".into(), short), p.value.fit.map_or("-".into(), |f| f.to_string()));
        vec![short(&p.uid), item, p.name, fit, p.value.background.unwrap_or("-".into())]
    });
    table(&["PRESET", "ITEM", "NAME", "FIT", "BACKGROUND"], rows)
}
