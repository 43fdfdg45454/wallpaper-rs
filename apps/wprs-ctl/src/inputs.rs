use crate::cli::JSON;
use crate::show::{self, list};
use anyhow::Result;
use clap::{Subcommand, ValueEnum};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};
use wprs_client::{Client, parse};
use wprs_core::model::{Input, InputsReply, OutputState, State};

#[derive(Subcommand, Debug)]
pub enum Inputs {
    #[command(about = "Let a scene use an input whenever it is shown")]
    Grant {
        #[arg(help = "Package id of the scene")]
        scene: String,
        input: Kind,
    },
    #[command(
        about = "Take an input back from a scene; it stops at once and clients stop asking until it is granted"
    )]
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
    let declined = |(scene, inputs): (&String, &BTreeSet<Input>)| {
        format!("declined for {scene}: {}", list(inputs.iter().map(|i| i.as_str())))
    };
    lines.extend(reply.declined.iter().map(declined));
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

const AUDIO: &str = "the sound that plays (turned into levels on this computer, never recorded or sent)";
const POINTER: &str = "the pointer (only its latest position over the screen, never kept)";
const SETTLE: Duration = Duration::from_secs(1);

pub async fn asking(
    client: &Client,
    output: &str,
    action: impl Future<Output = zbus::Result<()>>,
) -> Result<()> {
    let since = client.daemon.serial().await?;
    action.await.map_err(wprs_client::explain)?;
    if let Ok(asks) = asks(client, output, since).await {
        asks.iter().for_each(|ask| eprintln!("{}", show::clean(ask)));
    }
    Ok(())
}

async fn asks(client: &Client, output: &str, mut serial: u64) -> Result<BTreeSet<String>> {
    let ours = |name: &String| output.is_empty() || name == output;
    let state: State = parse(&client.daemon.get_state().await?)?;
    let scene =
        |o: &OutputState| o.image.as_ref().is_some_and(|p| p.extension().is_some_and(|e| e == "wpak"));
    if !state.outputs.iter().any(|(name, o)| ours(name) && scene(o)) {
        return Ok(BTreeSet::new());
    }
    let deadline = Instant::now() + SETTLE;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let (now, topics) = client.daemon.wait_changed(serial, left.as_millis() as u32).await?;
        if now == serial || topics.iter().any(|t| t == "inputs") {
            break;
        }
        serial = now;
    }
    let reply: InputsReply = parse(&client.inputs.get_inputs().await?)?;
    let decided = |scene: &String, input: Input| {
        [&reply.grants, &reply.declined].iter().any(|m| m.get(scene).is_some_and(|set| set.contains(&input)))
    };
    let shown = reply.shown.iter().filter(|(name, _)| ours(name)).map(|(_, shown)| shown);
    let asks = shown.flat_map(|s| {
        let title = if s.title.is_empty() { &s.scene } else { &s.title };
        let used = [(s.audio, Input::Audio, AUDIO), (s.pointer, Input::Pointer, POINTER)];
        let open = used.into_iter().filter(|(on, input, _)| *on && !decided(&s.scene, *input));
        open.map(move |(_, input, what)| {
            let grant = format!("wprs-ctl input grant {} {}", s.scene, input.as_str());
            format!("{title} follows {what} once you allow it: {grant}")
        })
    });
    Ok(asks.collect())
}
