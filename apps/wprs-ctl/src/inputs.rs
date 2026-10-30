use crate::cli::JSON;
use crate::show::{self, list};
use crate::usage;
use anyhow::{Result, bail};
use clap::{Subcommand, ValueEnum};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};
use wprs_client::{Client, error_name, parse};
use wprs_core::dbus::error::UNCONFIRMED;
use wprs_core::model::{Input, InputsReply, OutputState, SceneInputs, State};

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

fn status(reply: InputsReply) -> String {
    let on = |on| if on { "on" } else { "off" };
    let mut lines =
        vec![format!("audio capture: {}", on(reply.capturing)), format!("pointer: {}", on(reply.tracking))];
    let inputs = |set: &BTreeSet<Input>| match set.is_empty() {
        true => "none".to_owned(),
        false => list(set.iter().map(|i| i.as_str())),
    };
    let granted = |(scene, set): (&String, &BTreeSet<Input>)| {
        format!("granted to some version or signer of {scene}: {}", inputs(set))
    };
    lines.extend(reply.grants.iter().map(granted));
    let declined =
        |(scene, set): (&String, &BTreeSet<Input>)| format!("declined for {scene}: {}", inputs(set));
    lines.extend(reply.declined.iter().map(declined));
    let asked = |(scene, set): (&String, &BTreeSet<Input>)| {
        format!("asked in a desktop notification for {scene}: {}", inputs(set))
    };
    lines.extend(reply.asking.iter().map(asked));
    let quiet = |(scene, set): (&String, &BTreeSet<Input>)| {
        format!("not asked again for a few minutes for {scene}: {}", inputs(set))
    };
    lines.extend(reply.quiet.iter().map(quiet));
    lines.extend(reply.shown.iter().map(|(output, shown)| {
        let used =
            [(shown.audio, Input::Audio), (shown.pointer, Input::Pointer), (shown.clicks, Input::Clicks)];
        let used = used.into_iter();
        let used: BTreeSet<Input> = used.filter_map(|(on, input)| on.then_some(input)).collect();
        format!(
            "{output} shows {} using {} (allowed: {})",
            shown.scene,
            inputs(&used),
            inputs(&shown.granted)
        )
    }));
    let follows = |output: &String| format!("{output} shows an image whose parallax follows the pointer");
    lines.extend(reply.parallax.iter().map(follows));
    let screens = |set: &BTreeSet<String>| if set.is_empty() { "none".to_owned() } else { list(set.iter()) };
    lines.push(format!("clicks reach the wallpaper on: {}", screens(&reply.clicks)));
    lines.push(format!("taking clicks now: {}", screens(&reply.clicking)));
    lines.join("\n") + "\n"
}

const AUDIO: &str = "the sound that plays (turned into levels on this computer, never recorded or sent)";
const POINTER: &str = "the pointer (only its latest position over the screen, never kept)";
const CLICKS: &str = "clicks and taps on the screen (only real ones, never kept; keys are never read)";
const SLOW: &str = "its particles run on the processor, so fewer of them are shown";
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
    let mut after: State = parse(&client.daemon.get_state().await?)?;
    let unseen = |s: &State| {
        s.outputs.iter().any(|(n, o)| ours(n) && scene(o) && o.frames == 0 && o.particles.is_empty())
    };
    let deadline = Instant::now() + SETTLE;
    while let Some(left) = deadline.checked_duration_since(Instant::now()).filter(|_| unseen(&after)) {
        client.daemon.wait_changed(after.serial, left.as_millis().min(100) as u32).await?;
        after = parse(&client.daemon.get_state().await?)?;
    }
    let slow = after.outputs.iter().filter(|(name, o)| ours(name) && o.particles == "cpu");
    let slow = slow.map(|(name, _)| format!("{name}: {SLOW}"));
    let declined =
        |scene: &String, input: Input| reply.declined.get(scene).is_some_and(|set| set.contains(&input));
    let twins = |s: &SceneInputs| reply.shown.values().any(|o| o.scene == s.scene && o.key != s.key);
    let shown = reply.shown.iter().filter(|(name, _)| ours(name));
    let asks = shown.flat_map(|(name, s)| {
        let title = if s.title.is_empty() { &s.scene } else { &s.title };
        let used = [
            (s.audio, Input::Audio, AUDIO),
            (s.pointer, Input::Pointer, POINTER),
            (s.clicks, Input::Clicks, CLICKS),
        ];
        let decided = move |input: Input| s.granted.contains(&input) || declined(&s.scene, input);
        let open = used.into_iter().filter(move |(on, input, _)| *on && !decided(*input));
        let output = if twins(s) { format!(" -o {name}") } else { String::new() };
        let screen = !reply.clicks.contains(name.as_str());
        open.map(move |(_, input, what)| {
            let mut grant = format!("wprs-ctl input grant {} {}{output}", s.scene, input.as_str());
            if input == Input::Clicks && screen {
                grant += &format!(" && wprs-ctl input clicks on -o {name}");
            }
            format!("{title} follows {what} once you allow it: {grant}")
        })
    });
    Ok(asks.chain(slow).collect())
}
