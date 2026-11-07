use crate::author::{DIR, POINTER};
use crate::steps::value;
use crate::told::{fill, told};
use clap::{ArgAction, Subcommand};
use std::path::PathBuf;
use wprs_editor::actions::{Action, KeyframeEdit, TimelineEdit, TimelineEvent};

#[derive(Subcommand, Debug)]
pub enum Keying {
    #[command(
        about = "Add or replace a keyframe of an animatable value, remove it with --remove or move it with --from; --base makes the keyframes relative, --in-handle and --out-handle bend the curve between keys and --straighten makes it straight again"
    )]
    Keyframe {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = POINTER)]
        pointer: String,
        #[arg(
            allow_negative_numbers = true,
            help = "Seconds on the timeline, or on the scene clock without one"
        )]
        time: f64,
        #[arg(allow_hyphen_values = true, help = "JSON value; the current value when omitted")]
        value: Option<String>,
        #[arg(long, help = "Easing towards the next keyframe, such as in_out_sine, or [x1,y1,x2,y2]")]
        ease: Option<String>,
        #[arg(long, help = "Timeline that drives the keyframes; add it first with scene timeline --add")]
        timeline: Option<String>,
        #[arg(
            long,
            allow_hyphen_values = true,
            help = "JSON value the keyframes are added to, making them relative, such as 120 or [960,540]"
        )]
        base: Option<String>,
        #[arg(
            long,
            conflicts_with = "base",
            help = "Drop the base, so that the keyframes are the value again"
        )]
        absolute: bool,
        #[arg(long, allow_hyphen_values = true, value_name = "DT,DV", value_parser = pair, help = HANDLE_IN)]
        in_handle: Option<[f64; 2]>,
        #[arg(long, allow_hyphen_values = true, value_name = "DT,DV", value_parser = pair, help = HANDLE_OUT)]
        out_handle: Option<[f64; 2]>,
        #[arg(long, help = "Remove the keyframe at this time")]
        remove: bool,
        #[arg(
            long,
            conflicts_with_all = ["value", "ease", "base", "absolute", "in_handle", "out_handle", "remove", "from"],
            help = "Straighten the curve to the next keyframe, dropping only the handles or easing that bend it"
        )]
        straighten: bool,
        #[arg(
            allow_negative_numbers = true,
            long,
            value_name = "TIME",
            help = "Move the keyframe at this time to TIME, keeping its value and easing"
        )]
        from: Option<f64>,
    },
    #[command(
        about = "Add, change, rename or remove a timeline; renaming or removing it also moves or frees the values it drives"
    )]
    Timeline {
        #[arg(help = DIR)]
        dir: PathBuf,
        #[arg(help = "Id of the timeline")]
        id: String,
        #[arg(long, help = "Add it, of 5 s unless --duration or --frames and --fps say otherwise")]
        add: bool,
        #[arg(long, conflicts_with = "add", help = "Remove it")]
        remove: bool,
        #[arg(long = "rename", value_name = "ID", help = "A new id")]
        to: Option<String>,
        #[arg(allow_negative_numbers = true, long, help = "Length in seconds")]
        duration: Option<f64>,
        #[arg(allow_negative_numbers = true, long, help = "Frames a second, from 1 to 240, with --frames")]
        fps: Option<f64>,
        #[arg(long, help = "Length in frames, with --fps")]
        frames: Option<u32>,
        #[arg(long, action = ArgAction::Set, help = "Blend the end of a repeating timeline into its start")]
        wrap: Option<bool>,
        #[arg(id = "loop", long = "loop", value_parser = ["once", "repeat", "pingpong"], help = "How it plays")]
        repeat: Option<String>,
        #[arg(long, allow_hyphen_values = true, help = "Speed, from -100 to 100; below 0 plays backwards")]
        speed: Option<f64>,
        #[arg(allow_negative_numbers = true, long, help = "Seconds before it starts")]
        delay: Option<f64>,
        #[arg(long, action = ArgAction::Set, help = "Start on its own; false waits for its trigger")]
        autoplay: Option<bool>,
        #[arg(long, help = "What starts it, such as click or start; empty removes it")]
        trigger: Option<String>,
        #[arg(long = "event", value_name = "T:NAME", value_parser = event, help = "Add an event that fires at T seconds; repeat it for more")]
        add_events: Vec<TimelineEvent>,
        #[arg(
            long = "drop-event",
            value_name = "NAME[@T]",
            help = "Remove the events of that name, or only the one at T seconds"
        )]
        drop_events: Vec<String>,
        #[arg(long, help = "Remove every event first")]
        clear_events: bool,
    },
}

pub(crate) const HANDLE_IN: &str = "Bézier handle into this keyframe: seconds before it, up to the previous keyframe, and how much less the value is there; between keys of a position, color or other list it is a share of the way from the previous key (0) to this one (1), in value units only when both keys are single numbers";
pub(crate) const HANDLE_OUT: &str = "Bézier handle out of this keyframe: seconds after it, up to the next keyframe, and how much more the value is there; between keys of a position, color or other list it is a share of the way from this key (0) to the next (1), in value units only when both keys are single numbers";

fn event(text: &str) -> Result<TimelineEvent, String> {
    let (t, name) = text.split_once(':').ok_or_else(|| fill(told("%1 is not T:NAME"), &[&text]))?;
    let t = t.trim().parse().map_err(|_| fill(told("%1 is not a number of seconds"), &[&t]))?;
    Ok(TimelineEvent { t, name: name.trim().into() })
}

pub(crate) fn pair(text: &str) -> Result<[f64; 2], String> {
    let parsed =
        text.split_once(',').and_then(|(a, b)| Some([a.trim().parse().ok()?, b.trim().parse().ok()?]));
    parsed.ok_or_else(|| fill(told("%1 is not DT,DV"), &[&text]))
}

pub(crate) fn action(command: Keying) -> (PathBuf, Action) {
    match command {
        Keying::Keyframe {
            dir,
            pointer,
            time,
            value: v,
            ease,
            timeline,
            remove,
            from,
            base,
            absolute,
            in_handle,
            out_handle,
            straighten,
        } => {
            let mut key = KeyframeEdit::at(&pointer, time);
            (key.value, key.ease) = (v.as_deref().map(value), ease.as_deref().map(value));
            (key.timeline, key.remove, key.from) = (timeline, remove, from);
            (key.base, key.absolute) = (base.as_deref().map(value), absolute);
            (key.in_handle, key.out_handle, key.straighten) = (in_handle, out_handle, straighten);
            (dir, Action::Keyframe(key))
        }
        Keying::Timeline {
            dir,
            id,
            add,
            remove,
            to,
            duration,
            fps,
            frames,
            wrap,
            repeat,
            speed,
            delay,
            autoplay,
            trigger,
            add_events,
            drop_events,
            clear_events,
        } => {
            let repeat = repeat.and_then(|r| serde_json::from_value(serde_json::json!(r)).ok());
            let edit = TimelineEdit {
                id,
                add,
                remove,
                to,
                duration,
                fps,
                frames,
                wrap,
                repeat,
                speed,
                delay,
                autoplay,
                trigger,
                add_events,
                drop_events,
                clear_events,
            };
            (dir, Action::Timeline(edit))
        }
    }
}
