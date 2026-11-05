use crate::author::DIR;
use anyhow::{Result, bail};
use clap::Subcommand;
use serde_json::{Value, json};
use std::path::PathBuf;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_scene::MEDIA_TRIGGERS;

const NAMES: &str = "start, beat, prop:ID, event:NAME, click:LAYER, enter:LAYER, leave:LAYER, media:play (the media player starts playing), media:pause (it stops), media:track (a new track) or media:cover (a new cover)";

#[derive(Subcommand, Debug)]
pub enum Triggering {
    #[command(
        about = "Show or set what starts a timeline or a sound layer; --list shows the triggers the editor offers for it"
    )]
    Trigger {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = "Id of a timeline or of a sound layer")]
        target: String,
        #[arg(help = NAMES)]
        trigger: Option<String>,
        #[arg(long, conflicts_with = "trigger", help = "Take the trigger away")]
        none: bool,
        #[arg(long, conflicts_with_all = ["trigger", "none"], help = "List the triggers offered for it")]
        list: bool,
    },
}

fn place(doc: &Value, target: &str) -> Result<String> {
    let timelines = doc.pointer("/scene/timelines").and_then(Value::as_array).into_iter().flatten();
    if let Some(i) = timelines.clone().position(|t| t["id"] == target) {
        return Ok(format!("/scene/timelines/{i}/trigger"));
    }
    let layers = wprs_editor::layers::walk(doc);
    match layers.into_iter().find(|(_, l)| l["id"] == target) {
        Some((pointer, l)) if l["type"] == "sound" => Ok(format!("{pointer}/play_on")),
        Some(_) => bail!("{target} is not a sound layer; only timelines and sound layers have triggers"),
        None => bail!("there is no timeline or sound layer {target}"),
    }
}

fn known(trigger: &str) -> Result<()> {
    match trigger.split_once(':') {
        Some(("media", what)) if !MEDIA_TRIGGERS.contains(&what) => {
            bail!("{what} is not a media trigger; use media:{}", MEDIA_TRIGGERS.join(", media:"))
        }
        Some((_, "")) => bail!("{trigger} names nothing after the colon"),
        _ => Ok(()),
    }
}

pub fn run(command: Triggering) -> Result<()> {
    let Triggering::Trigger { dir, target, trigger, none, list } = command;
    let project = Project::open(&dir)?;
    let pointer = place(&project.doc, &target)?;
    if list {
        let offered =
            wprs_editor::settings::candidates(&project.doc, "trigger", &pointer).unwrap_or_default();
        offered.iter().filter_map(Value::as_str).for_each(|t| println!("{t}"));
        return Ok(());
    }
    let action = match trigger {
        _ if none => Action::Remove { pointer },
        Some(trigger) => {
            known(&trigger)?;
            Action::Set { pointer, value: json!(trigger) }
        }
        None => {
            println!("{}", project.doc.pointer(&pointer).and_then(Value::as_str).unwrap_or("none"));
            return Ok(());
        }
    };
    crate::steps::edit(&dir, action)
}
