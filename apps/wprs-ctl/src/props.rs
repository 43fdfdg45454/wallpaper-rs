use crate::cli::{JSON, OUTPUT};
use crate::show::{self, table};
use anyhow::{Context, Result};
use clap::Subcommand;
use serde_json::Value;
use wprs_client::Client;
use wprs_core::model::SceneProperties;

#[derive(Subcommand, Debug)]
pub enum Prop {
    #[command(about = "List the properties of the scene shown on an output, with their values")]
    Ls {
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Print the value of one property")]
    Get {
        id: String,
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
    },
    #[command(about = "Set a property; the value is JSON or plain text such as #ff8800")]
    Set {
        id: String,
        value: String,
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
    },
    #[command(about = "Apply a preset of the scene")]
    Preset {
        preset: String,
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
    },
    #[command(about = "Forget the values set for the scene, back to its defaults")]
    Reset {
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
    },
}

pub fn value(text: &str) -> String {
    match serde_json::from_str::<Value>(text) {
        Ok(_) => text.to_owned(),
        Err(_) => Value::String(text.to_owned()).to_string(),
    }
}

pub async fn run(client: &Client, command: Prop) -> Result<()> {
    let daemon = &client.daemon;
    match command {
        Prop::Ls { output, json } => show::reply(daemon.get_scene_properties(&output).await?, json, list)?,
        Prop::Get { id, output } => {
            let reply: SceneProperties = wprs_client::parse(&daemon.get_scene_properties(&output).await?)?;
            let found =
                reply.values.get(&id).with_context(|| format!("{} has no property {id}", reply.package))?;
            println!("{found}");
        }
        Prop::Set { id, value: text, output } => {
            daemon.set_scene_property(&output, &id, &value(&text)).await?
        }
        Prop::Preset { preset, output } => daemon.apply_scene_preset(&output, &preset).await?,
        Prop::Reset { output } => daemon.reset_scene_properties(&output).await?,
    }
    Ok(())
}

fn list(reply: SceneProperties) -> String {
    let fields = reply.fields.iter().filter(|f| !["group", "label"].contains(&f.kind.as_str()));
    let rows = fields.map(|f| {
        let label = if f.visible { f.label.clone() } else { format!("{} (hidden)", f.label) };
        let value = if f.value.is_null() { String::new() } else { f.value.to_string() };
        let range = match (f.min, f.max) {
            (Some(lo), Some(hi)) => format!("{lo} to {hi}"),
            _ => f.options.iter().map(|o| o.value.as_str()).collect::<Vec<_>>().join(", "),
        };
        vec![f.key.clone(), label, f.kind.clone(), value, range]
    });
    let presets = reply.preset_labels.iter().map(|p| p.value.as_str()).collect::<Vec<_>>().join(", ");
    let head = format!("{} ({})\n", reply.title, reply.package);
    let tail = if presets.is_empty() { String::new() } else { format!("presets: {presets}\n") };
    let warnings: String = reply.warnings.iter().map(|w| format!("warning: {w}\n")).collect();
    head + &table(&["PROPERTY", "LABEL", "TYPE", "VALUE", "RANGE"], rows) + &tail + &warnings
}
