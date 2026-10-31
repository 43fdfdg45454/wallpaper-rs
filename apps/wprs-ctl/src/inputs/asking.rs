use super::shown::{AUDIO, CLICKS, POINTER, SLOW};
use crate::show;
use anyhow::Result;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};
use wprs_client::{Client, parse};
use wprs_core::model::{Input, InputsReply, OutputState, SceneInputs, State};

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
