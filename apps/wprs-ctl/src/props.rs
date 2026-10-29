use crate::cli::{JSON, OUTPUT};
use crate::propsets::{self, PropPreset};
use crate::show::{self, table};
use crate::usage;
use anyhow::{Context, Result, ensure};
use clap::Subcommand;
use serde_json::Value;
use std::path::{Path, PathBuf};
use wprs_client::Client;
use wprs_core::model::SceneProperties;

#[derive(Subcommand, Debug)]
pub enum Prop {
    #[command(about = "List the settings of the package shown on an output, with their values")]
    Ls {
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Print the value of one setting")]
    Get {
        id: String,
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
    },
    #[command(about = "Set a setting to JSON or plain text such as #ff8800, asked for when left out; \
        file and folder settings take a path, which may be relative to the current folder")]
    Set {
        id: String,
        #[arg(allow_negative_numbers = true)]
        value: Option<String>,
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
    },
    #[command(about = "Forget the values set for the package, back to its defaults")]
    Reset {
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
    },
    #[command(about = "Press a button of the scene")]
    Press {
        id: String,
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
    },
    #[command(about = "Let a screen follow the shared values of every screen, or keep its own with --own")]
    Share {
        #[arg(short, long, help = "Output name such as DP-1")]
        output: String,
        #[arg(long, help = "Give the screen its own copy of the values")]
        own: bool,
    },
    #[command(subcommand, about = "Presets of the package and the ones you saved")]
    Preset(PropPreset),
}

pub fn value(text: &str) -> String {
    match serde_json::from_str::<Value>(text) {
        Ok(_) => text.to_owned(),
        Err(_) => Value::String(text.to_owned()).to_string(),
    }
}

pub fn path(text: &str, base: &Path) -> Result<String> {
    let text = serde_json::from_str::<String>(text).unwrap_or_else(|_| text.to_owned());
    ensure!(!text.trim().is_empty(), "a file or folder setting takes a path or null");
    let found = match Path::new(&text).is_absolute() {
        true => PathBuf::from(&text),
        false => std::fs::canonicalize(base.join(&text)).with_context(|| format!("cannot find {text}"))?,
    };
    Ok(Value::String(found.to_string_lossy().into_owned()).to_string())
}

pub async fn run(client: &Client, command: Prop) -> Result<()> {
    let daemon = &client.daemon;
    match command {
        Prop::Ls { output, json } => show::reply(daemon.get_scene_properties(&output).await?, json, list)?,
        Prop::Get { id, output } => {
            let reply: SceneProperties = wprs_client::parse(&daemon.get_scene_properties(&output).await?)?;
            let found = reply
                .values
                .get(&id)
                .ok_or_else(|| usage(format!("{} has no setting {id}", reply.package)))?;
            println!("{found}");
        }
        Prop::Set { id, value: text, output } => {
            let text = match text {
                Some(text) => text,
                None => crate::input::secret(&format!("{id}: "), false)?,
            };
            let reply: SceneProperties = wprs_client::parse(&daemon.get_scene_properties(&output).await?)?;
            let chosen =
                reply.fields.iter().any(|f| f.key == id && ["file", "folder"].contains(&f.kind.as_str()));
            let text = if chosen && text.trim() != "null" {
                path(&text, &std::env::current_dir()?)?
            } else {
                value(&text)
            };
            daemon.set_scene_property(&output, &id, &text).await?
        }
        Prop::Reset { output } => daemon.reset_scene_properties(&output).await?,
        Prop::Press { id, output } => daemon.press_scene_button(&output, &id).await?,
        Prop::Share { output, own } => daemon.share_scene_values(&output, !own).await?,
        Prop::Preset(command) => propsets::run(client, command).await?,
    }
    Ok(())
}

fn list(reply: SceneProperties) -> String {
    let fields = reply.fields.iter().filter(|f| !["group", "label", "divider"].contains(&f.kind.as_str()));
    let rows = fields.map(|f| {
        let state = [(!f.visible, "hidden"), (!f.enabled, "off")].into_iter().filter(|(on, _)| *on);
        let state: Vec<&str> = state.map(|(_, w)| w).collect();
        let label =
            if state.is_empty() { f.label.clone() } else { format!("{} ({})", f.label, state.join(", ")) };
        let value = match (f.secret, f.filled) {
            (true, true) => "(stored)".into(),
            _ if f.value.is_null() => String::new(),
            _ => f.value.to_string(),
        };
        let range = match (f.min, f.max) {
            (Some(lo), Some(hi)) => format!(
                "{lo} to {hi}{}",
                if f.unit.is_empty() { String::new() } else { format!(" {}", f.unit) }
            ),
            _ => show::list(f.options.iter().map(|o| o.value.to_string())),
        };
        vec![f.key.clone(), label, f.kind.clone(), value, range]
    });
    let scope = match (reply.output.is_empty(), reply.shared) {
        (true, _) => "every screen",
        (false, true) => "shared with every screen",
        (false, false) => "this screen only",
    };
    let head = format!("{} ({}, {scope})\n", reply.title, reply.package);
    let named = |p: &wprs_core::model::SceneOption| match p.value.as_str().unwrap_or_default() {
        id if id == p.label => id.to_owned(),
        id => format!("{id} ({})", p.label),
    };
    let names = |list: &[wprs_core::model::SceneOption]| show::list(list.iter().map(named));
    let presets = [
        ("presets", names(&reply.preset_labels)),
        ("saved", show::list(reply.user_presets.iter().map(|p| p.label.clone()))),
    ];
    let tail: String =
        presets.iter().filter(|(_, n)| !n.is_empty()).map(|(k, n)| format!("{k}: {n}\n")).collect();
    let warnings: String = reply.warnings.iter().map(|w| format!("warning: {w}\n")).collect();
    head + &table(&["SETTING", "LABEL", "TYPE", "VALUE", "RANGE"], rows) + &tail + &warnings
}
