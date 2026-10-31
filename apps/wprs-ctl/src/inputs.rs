use crate::cli::JSON;
use crate::show::{self, list};
use crate::usage;
use anyhow::{Result, bail};
use clap::{Subcommand, ValueEnum};
use std::collections::BTreeSet;
use wprs_client::{Client, error_name, parse};
use wprs_core::dbus::error::UNCONFIRMED;
use wprs_core::model::{Input, InputsReply};

mod asking;
mod shown;

pub use asking::asking;
use shown::status;

#[derive(Subcommand, Debug)]
pub enum Inputs {
    #[command(
        about = "Let the scene package shown now use an input whenever it is shown; another package with the same id does not get it"
    )]
    Grant {
        #[arg(help = "Package id of the scene")]
        scene: String,
        input: Kind,
        #[arg(
            short,
            long,
            help = "The screen whose package gets the input, when several show one with this id"
        )]
        output: Option<String>,
    },
    #[command(
        about = "Take an input back from a scene; it stops at once and clients stop asking until it is granted"
    )]
    Revoke {
        #[arg(help = "Package id of the scene")]
        scene: String,
        input: Kind,
    },
    #[command(
        about = "Show what each screen's scene may use and whether audio is captured and the pointer followed"
    )]
    Status {
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(
        about = "Let clicks and taps on a screen reach its wallpaper, for packages allowed the clicks input; off by default"
    )]
    Clicks {
        state: Switch,
        #[arg(short, long, help = "The screen; every screen when left out")]
        output: Option<String>,
    },
    #[command(about = "Report the pointer position in logical desktop coordinates")]
    Pointer {
        #[arg(allow_hyphen_values = true)]
        x: i32,
        #[arg(allow_hyphen_values = true)]
        y: i32,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum Kind {
    Audio,
    Pointer,
    Clicks,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum Switch {
    On,
    Off,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Audio => Input::Audio.as_str(),
            Kind::Pointer => Input::Pointer.as_str(),
            Kind::Clicks => Input::Clicks.as_str(),
        }
    }
}

pub async fn run(client: &Client, command: Inputs) -> Result<()> {
    let inputs = &client.inputs;
    match command {
        Inputs::Grant { scene, input, output } => {
            grant(client, &scene, input.name(), output.as_deref()).await?
        }
        Inputs::Revoke { scene, input } => inputs.revoke(&scene, input.name()).await?,
        Inputs::Status { json } => show::reply(inputs.get_inputs().await?, json, status)?,
        Inputs::Pointer { x, y } => inputs.set_pointer(x, y).await?,
        Inputs::Clicks { state, output } => {
            inputs.set_clicks(output.as_deref().unwrap_or_default(), matches!(state, Switch::On)).await?
        }
    }
    Ok(())
}

async fn grant(client: &Client, scene: &str, input: &str, output: Option<&str>) -> Result<()> {
    let reply: InputsReply = parse(&client.inputs.get_inputs().await?)?;
    let shown = reply.shown.iter().filter(|(name, s)| s.scene == scene && output.is_none_or(|o| o == *name));
    let keys: BTreeSet<&str> = shown.map(|(_, s)| s.key.as_str()).filter(|k| !k.is_empty()).collect();
    let sent = match (Vec::from_iter(keys).as_slice(), output) {
        ([key], _) => client.inputs.grant_package(scene, key, input).await,
        ([], Some(output)) => bail!("{output} does not show {scene}"),
        ([], None) => client.inputs.grant(scene, input).await,
        _ => {
            let outputs = reply.shown.iter().filter(|(_, s)| s.scene == scene).map(|(name, _)| name);
            let many =
                format!("more than one package with the id {scene} is shown; choose one with --output");
            return Err(usage(format!("{many} ({})", list(outputs))));
        }
    };
    match sent.map_err(wprs_client::explain) {
        Err(e) if error_name(&e) == Some(UNCONFIRMED) => {
            eprintln!("Asked in a desktop notification: the scene gets the input once you allow it there.");
            Ok(())
        }
        sent => sent,
    }
}
