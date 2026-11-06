use crate::cli::JSON;
use crate::input::path_arg;
use crate::show::{self, list, short, table};
use anyhow::Result;
use clap::Subcommand;
use std::path::Path;
use wprs_client::{Client, parse};
use wprs_core::model::{Profile as Model, ProfileReply, ProfilesReply};

#[derive(Subcommand, Debug)]
pub enum Profile {
    #[command(about = "List the profiles")]
    Ls {
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Show a profile")]
    Show {
        #[arg(help = "Name or uid of the profile")]
        profile: String,
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Save what every output shows as a profile and print its uid")]
    Save {
        #[arg(help = "Name of the new profile")]
        name: String,
        #[arg(long, help = "Leave the settings out of the profile")]
        no_options: bool,
    },
    #[command(about = "Activate a profile")]
    Use {
        #[arg(help = "Name or uid of the profile")]
        profile: String,
    },
    #[command(about = "Clear the active profile without changing anything else")]
    Off,
    #[command(about = "Rename a profile")]
    Rename {
        #[arg(help = "Name or uid of the profile")]
        profile: String,
        #[arg(help = "New name")]
        name: String,
    },
    #[command(about = "Delete a profile")]
    Rm {
        #[arg(help = "Name or uid of the profile")]
        profile: String,
    },
    #[command(about = "Print the automatic activation rules, one rule, or change a rule")]
    Auto {
        #[arg(help = "Name or uid of the profile")]
        profile: String,
        #[arg(help = "outputs, on_battery or priority")]
        key: Option<String>,
        #[arg(help = "New value of the rule; the rule is printed when left out")]
        value: Option<String>,
        #[arg(long, help = JSON)]
        json: bool,
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

pub(crate) fn named(text: &str) -> Option<(&str, &str)> {
    let splits: Vec<_> = text.match_indices(':').map(|(i, _)| (&text[..i], &text[i + 1..])).collect();
    let file = |item: &str| path_arg(item).is_ok_and(|path| Path::new(&path).is_file());
    splits.iter().rev().find(|(item, _)| file(item)).or(splits.first()).copied()
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
