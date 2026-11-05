use crate::author::{DIR, POINTER};
use crate::steps::value;
use clap::{ArgAction, Subcommand};
use std::path::PathBuf;
use wprs_editor::actions::{Action, KeyframeEdit, TimelineEdit};

#[derive(Subcommand, Debug)]
pub enum Keying {
    #[command(
        about = "Add or replace a keyframe of an animatable value, remove it with --remove or move it with --from; --base makes the keyframes relative and --in-handle and --out-handle bend the curve of a number"
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
    },
}

const HANDLE_IN: &str = "Bézier handle into this keyframe: seconds before it, up to the previous keyframe, and how much less the value is there";
const HANDLE_OUT: &str = "Bézier handle out of this keyframe: seconds after it, up to the next keyframe, and how much more the value is there";

fn pair(text: &str) -> Result<[f64; 2], String> {
    let parsed =
        text.split_once(',').and_then(|(a, b)| Some([a.trim().parse().ok()?, b.trim().parse().ok()?]));
    parsed.ok_or_else(|| format!("{text} is not DT,DV"))
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
        } => {
            let mut key = KeyframeEdit::at(&pointer, time);
            (key.value, key.ease) = (v.as_deref().map(value), ease.as_deref().map(value));
            (key.timeline, key.remove, key.from) = (timeline, remove, from);
            (key.base, key.absolute) = (base.as_deref().map(value), absolute);
            (key.in_handle, key.out_handle) = (in_handle, out_handle);
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
            };
            (dir, Action::Timeline(edit))
        }
    }
}
