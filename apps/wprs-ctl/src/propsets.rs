use crate::cli::{JSON, OUTPUT};
use crate::show::{self, table};
use anyhow::{Context, Result};
use clap::Subcommand;
use std::io::Read;
use std::path::PathBuf;
use wprs_client::Client;
use wprs_core::model::{SceneOption, SceneProperties};

#[derive(Subcommand, Debug)]
pub enum PropPreset {
    #[command(about = "List the presets of the package shown on an output and the ones you saved")]
    Ls {
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Save the current values under a name; saving the same name again replaces it")]
    Save {
        name: String,
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
    },
    #[command(about = "Apply a preset of the package or one you saved")]
    Load {
        preset: String,
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
    },
    #[command(about = "Rename a preset you saved")]
    Rename {
        preset: String,
        name: String,
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
    },
    #[command(about = "Delete a preset you saved")]
    Rm {
        preset: String,
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
    },
    #[command(about = "Write the current values, or those of a preset, to a small file or to the screen")]
    Export {
        preset: Option<String>,
        #[arg(short, long, help = "File to write; the screen when omitted")]
        file: Option<PathBuf>,
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
    },
    #[command(about = "Apply the values of an exported file, or save them as a preset with --save")]
    Import {
        #[arg(help = "File written by export, or - for the standard input")]
        file: PathBuf,
        #[arg(long, help = "Save the values as a preset of this name instead of applying them")]
        save: Option<String>,
        #[arg(short, long, default_value = "", help = OUTPUT)]
        output: String,
    },
}

async fn saved(client: &Client, output: &str, preset: &str) -> Result<String> {
    let reply: SceneProperties = wprs_client::parse(&client.daemon.get_scene_properties(output).await?)?;
    let named = |p: &&SceneOption| p.label.eq_ignore_ascii_case(preset) || p.value.as_str() == Some(preset);
    let found = reply.user_presets.iter().find(named).and_then(|p| p.value.as_str().map(String::from));
    found.with_context(|| format!("{} has no saved preset {preset:?}", reply.package))
}

pub async fn run(client: &Client, command: PropPreset) -> Result<()> {
    let daemon = &client.daemon;
    match command {
        PropPreset::Ls { output, json } => {
            show::reply(daemon.get_scene_properties(&output).await?, json, list)?
        }
        PropPreset::Save { name, output } => drop(daemon.save_scene_preset(&output, &name).await?),
        PropPreset::Load { preset, output } => daemon.apply_scene_preset(&output, &preset).await?,
        PropPreset::Rename { preset, name, output } => {
            daemon.rename_scene_preset(&saved(client, &output, &preset).await?, &name).await?
        }
        PropPreset::Rm { preset, output } => {
            daemon.delete_scene_preset(&saved(client, &output, &preset).await?).await?
        }
        PropPreset::Export { preset, file, output } => {
            let text =
                daemon.export_scene_values(&output, preset.as_deref().unwrap_or_default()).await? + "\n";
            match file {
                Some(file) => {
                    std::fs::write(&file, text).with_context(|| format!("cannot write {}", file.display()))?
                }
                None => print!("{text}"),
            }
        }
        PropPreset::Import { file, save, output } => {
            let mut text = String::new();
            match file.to_str() {
                Some("-") => drop(std::io::stdin().read_to_string(&mut text)?),
                _ => {
                    text = std::fs::read_to_string(&file)
                        .with_context(|| format!("cannot read {}", file.display()))?
                }
            }
            let warnings =
                daemon.import_scene_values(&output, &text, save.as_deref().unwrap_or_default()).await?;
            warnings.iter().for_each(|w| eprintln!("warning: {}", show::clean(w)));
        }
    }
    Ok(())
}

fn list(reply: SceneProperties) -> String {
    let own = reply.preset_labels.iter().map(|p| (p, "package"));
    let rows = own.chain(reply.user_presets.iter().map(|p| (p, "saved")));
    let rows = rows
        .map(|(p, from)| vec![p.value.as_str().unwrap_or_default().to_owned(), p.label.clone(), from.into()]);
    table(&["PRESET", "NAME", "FROM"], rows)
}
