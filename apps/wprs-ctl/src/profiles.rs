use crate::cli::JSON;
use crate::input::{one, pairs, path_arg};
use crate::show::{self, list, short, table};
use anyhow::{Context, Result};
use clap::Subcommand;
use std::path::Path;
use wprs_client::{Client, parse};
use wprs_core::model::{PresetsReply, Profile as Model, ProfileReply, ProfilesReply};

#[derive(Subcommand, Debug)]
pub enum Profile {
    #[command(about = "List the profiles")]
    Ls {
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Show a profile")]
    Show {
        profile: String,
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Save what every output shows as a profile and print its uid")]
    Save {
        name: String,
        #[arg(long, help = "Leave the settings out of the profile")]
        no_options: bool,
    },
    #[command(about = "Activate a profile")]
    Use { profile: String },
    #[command(about = "Clear the active profile without changing anything else")]
    Off,
    #[command(about = "Rename a profile")]
    Rename { profile: String, name: String },
    #[command(about = "Delete a profile")]
    Rm { profile: String },
    #[command(about = "Print the automatic activation rules, one rule, or change a rule")]
    Auto {
        profile: String,
        #[arg(help = "outputs, on_battery or priority")]
        key: Option<String>,
        value: Option<String>,
        #[arg(long, help = JSON)]
        json: bool,
    },
}

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

pub async fn run(client: &Client, command: Profile) -> Result<()> {
    let profiles = &client.profiles;
    match command {
        Profile::Ls { json } => show::reply(profiles.list().await?, json, list_profiles)?,
        Profile::Show { profile, json } => {
            show::reply(profiles.get(&profile).await?, json, |r: ProfileReply| details(r.profile))?
        }
        Profile::Save { name, no_options } => println!("{}", profiles.save(&name, !no_options).await?),
        Profile::Use { profile } => profiles.activate(&profile).await?,
        Profile::Off => profiles.activate("").await?,
        Profile::Rename { profile, name } => profiles.rename(&profile, &name).await?,
        Profile::Rm { profile } => profiles.delete(&profile).await?,
        Profile::Auto { profile, key: Some(key), value: Some(value), .. } => {
            profiles.set_auto(&profile, &key, &value).await?
        }
        Profile::Auto { profile, key, json, .. } => {
            let reply = profiles.get(&profile).await?;
            let text = match json {
                true => show::raw(&reply)?,
                false => show::pick(auto(&parse::<ProfileReply>(&reply)?.profile), key.as_deref())?,
            };
            print!("{text}")
        }
    }
    Ok(())
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

fn named(text: &str) -> Option<(&str, &str)> {
    let splits: Vec<_> = text.match_indices(':').map(|(i, _)| (&text[..i], &text[i + 1..])).collect();
    let file = |item: &str| path_arg(item).is_ok_and(|path| Path::new(&path).is_file());
    splits.iter().rev().find(|(item, _)| file(item)).or(splits.first()).copied()
}

async fn scope(client: &Client, item: &str) -> Result<String> {
    if item == "-" { Ok(String::new()) } else { one(client, item).await }
}

pub fn auto(profile: &Model) -> Vec<(&'static str, String)> {
    vec![
        ("outputs", profile.auto.outputs.as_deref().map_or(String::new(), |o| o.join(","))),
        ("on_battery", profile.auto.on_battery.map_or(String::new(), |b| b.to_string())),
        ("priority", profile.priority.to_string()),
    ]
}

pub fn list_profiles(reply: ProfilesReply) -> String {
    let rows = reply.profiles.into_iter().map(|p| {
        let rules =
            auto(&p).into_iter().take(2).filter(|(_, v)| !v.is_empty()).map(|(k, v)| format!("{k}={v}"));
        let (active, rules) = (if p.active { "yes" } else { "" }.into(), rules.collect::<Vec<_>>().join(" "));
        vec![short(&p.uid), p.name, active, p.priority.to_string(), rules]
    });
    table(&["PROFILE", "NAME", "ACTIVE", "PRIORITY", "AUTO"], rows)
}

pub fn details(profile: Model) -> String {
    let labelled =
        |label: &str, uid: &Option<String>| uid.as_deref().map(|uid| format!("{label} {}", short(uid)));
    let sections = profile.outputs.iter().map(|(name, o)| {
        let (path, fit) = (o.path.as_ref().map(|p| p.display().to_string()), o.fit.map(|f| f.to_string()));
        let parts =
            [labelled("item", &o.item), path, labelled("playlist", &o.playlist), fit, o.background.clone()];
        let parts = parts.into_iter().chain([labelled("preset", &o.preset)]).flatten();
        (format!("output {}", if name.is_empty() { "default" } else { name }), list(parts))
    });
    let options = profile.options.iter().map(|(key, value)| (format!("option {key}"), value.clone()));
    let head = [
        ("profile", profile.uid.clone()),
        ("name", profile.name.clone()),
        ("active", profile.active.to_string()),
    ];
    let head = head.into_iter().chain(auto(&profile)).map(|(k, v)| (k.to_string(), v));
    show::lines(head.chain(sections).chain(options))
}

pub fn presets(reply: PresetsReply) -> String {
    let rows = reply.presets.into_iter().map(|p| {
        let (item, fit) =
            (p.item.as_deref().map_or("-".into(), short), p.value.fit.map_or("-".into(), |f| f.to_string()));
        vec![short(&p.uid), item, p.name, fit, p.value.background.unwrap_or("-".into())]
    });
    table(&["PRESET", "ITEM", "NAME", "FIT", "BACKGROUND"], rows)
}
