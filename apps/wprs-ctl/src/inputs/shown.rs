use crate::show::list;
use std::collections::BTreeSet;
use wprs_core::model::{Input, InputsReply};

pub(super) fn status(reply: InputsReply) -> String {
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
        let used = used.into_iter().chain([(shown.network, Input::Network)]);
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

pub(super) const AUDIO: &str =
    "the sound that plays (turned into levels on this computer, never recorded or sent)";

pub(super) const POINTER: &str = "the pointer (only its latest position over the screen, never kept)";

pub(super) const CLICKS: &str =
    "clicks and taps on the screen (only real ones, never kept; keys are never read)";

pub(super) const SLOW: &str = "its particles run on the processor, so fewer of them are shown";

pub(super) const NETWORK: &str =
    "the network, to reach the sites it names (they can learn that it runs on this computer)";
