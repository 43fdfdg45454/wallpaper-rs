use crate::cli::JSON;
use crate::show::{self, list};
use anyhow::Result;
use clap::{Subcommand, ValueEnum};
use std::collections::BTreeSet;
use wprs_client::Client;
use wprs_core::model::{Input, InputsReply};

#[derive(Subcommand, Debug)]
pub enum Inputs {
    #[command(about = "Let a scene use an input whenever it is shown")]
    Grant {
        #[arg(help = "Package id of the scene")]
        scene: String,
        input: Kind,
    },
    #[command(about = "Take an input back from a scene; it stops at once")]
    Revoke {
        #[arg(help = "Package id of the scene")]
        scene: String,
        input: Kind,
    },
    #[command(about = "Show the grants and whether audio is captured and the pointer followed")]
    Status {
        #[arg(long, help = JSON)]
        json: bool,
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
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Audio => Input::Audio.as_str(),
            Kind::Pointer => Input::Pointer.as_str(),
        }
    }
}

pub async fn run(client: &Client, command: Inputs) -> Result<()> {
    let inputs = &client.inputs;
    match command {
        Inputs::Grant { scene, input } => inputs.grant(&scene, input.name()).await?,
        Inputs::Revoke { scene, input } => inputs.revoke(&scene, input.name()).await?,
        Inputs::Status { json } => show::reply(inputs.get_inputs().await?, json, status)?,
        Inputs::Pointer { x, y } => inputs.set_pointer(x, y).await?,
    }
    Ok(())
}

fn status(reply: InputsReply) -> String {
    let on = |on| if on { "on" } else { "off" };
    let mut lines =
        vec![format!("audio capture: {}", on(reply.capturing)), format!("pointer: {}", on(reply.tracking))];
    let granted = |(scene, inputs): (&String, &BTreeSet<Input>)| {
        format!("granted to {scene}: {}", list(inputs.iter().map(|i| i.as_str())))
    };
    lines.extend(reply.grants.iter().map(granted));
    lines.extend(reply.shown.iter().map(|(output, shown)| {
        let used = [(shown.audio, "audio"), (shown.pointer, "pointer")].into_iter();
        format!(
            "{output} shows {} using {}",
            shown.scene,
            list(used.filter_map(|(on, name)| on.then_some(name)))
        )
    }));
    let follows = |output: &String| format!("{output} shows an image whose parallax follows the pointer");
    lines.extend(reply.parallax.iter().map(follows));
    lines.join("\n") + "\n"
}
