use crate::sowed::pair;
use crate::told::{fill, told};
use clap::{ArgAction, Args, Subcommand};
use serde_json::{Map, Value, json};
use wprs_editor::actions::{ParticleEdit as P, SowEdit as S};

fn quad(text: &str) -> Result<String, String> {
    let n: Vec<f64> = text.split(',').filter_map(|v| v.trim().parse().ok()).collect();
    match n.len() == 4 || text == "none" {
        true => Ok(text.to_owned()),
        false => Err(fill(told("%1 is not none or FADE_IN,FULL,FADE_OUT,GONE"), &[&text])),
    }
}

fn waves(text: &str) -> Result<[[f64; 2]; 2], String> {
    let (on, off) = text
        .split_once(':')
        .ok_or_else(|| fill(told("%1 is not ON_MIN,ON_MAX:OFF_MIN,OFF_MAX"), &[&text]))?;
    Ok([pair(on)?, pair(off)?])
}

#[derive(Subcommand, Debug)]
pub enum Seeding {
    #[command(about = "Make an emitter burst, pause, start late, stop, follow the pointer or wait for sound")]
    Emitter {
        #[arg(help = "Index of the emitter, 0 first")]
        index: usize,
        #[arg(long, help = "Particles started at once")]
        burst: Option<u32>,
        #[arg(allow_negative_numbers = true, long, requires = "burst", help = "Seconds between bursts")]
        every: Option<f64>,
        #[arg(long, requires = "burst", value_parser = ["beat", "click"], help = "Burst on a beat or a click")]
        on: Option<String>,
        #[arg(long, value_name = "ON_MIN,ON_MAX:OFF_MIN,OFF_MAX", value_parser = waves, allow_hyphen_values = true, help = "Seconds it emits and seconds it rests, picked between the two")]
        periodic: Option<[[f64; 2]; 2]>,
        #[arg(allow_negative_numbers = true, long, help = "Seconds it emits before it stops")]
        duration: Option<f64>,
        #[arg(allow_negative_numbers = true, long, help = "Seconds it waits before it starts")]
        delay: Option<f64>,
        #[arg(long, action = ArgAction::Set, help = "true moves the emitter with the pointer")]
        follow: Option<bool>,
        #[arg(long, action = ArgAction::Set, help = "true emits only while sound plays")]
        audio_gate: Option<bool>,
        #[arg(
            allow_negative_numbers = true,
            long,
            help = "Share from 0 to 1 of the speed of the emitter the particles take"
        )]
        inherit: Option<f64>,
        #[arg(long, value_parser = ["burst", "periodic", "duration", "delay", "follow", "audio_gate", "inherit"], help = "Remove a value")]
        clear: Vec<String>,
    },
    #[command(about = "Let the force at INDEX act only in a window of the life of a particle, or none")]
    Window {
        #[arg(help = "Index of the force, 0 first")]
        index: usize,
        #[arg(value_parser = quad, help = "Shares of the life from 0 to 1 where it fades in, is full, fades out and is gone, or none")]
        window: String,
    },
    #[command(about = "Rename the system, with the layers and systems that use it")]
    Rename {
        #[arg(help = "New id of the system")]
        to: String,
    },
}

#[derive(Args, Debug)]
pub struct Tint {
    #[arg(long, value_name = "MIN,MAX", value_parser = pair, help = "Hues in degrees from 0 to 360 that new particles pick from")]
    pub hsv: Option<[f64; 2]>,
    #[arg(long, value_name = "MIN,MAX", value_parser = pair, requires = "hsv", help = "Saturation from 0 to 1 of the hues; 1,1 when omitted")]
    pub saturation: Option<[f64; 2]>,
    #[arg(long, value_name = "MIN,MAX", value_parser = pair, requires = "hsv", help = "Value from 0 to 1 of the hues; 1,1 when omitted")]
    pub value: Option<[f64; 2]>,
    #[arg(long, requires = "hsv", help = "Pick the hue among this many steps, from 1 to 360")]
    pub hue_steps: Option<u32>,
    #[arg(
        long,
        value_name = "COLOR,COLOR...",
        value_delimiter = ',',
        conflicts_with = "hsv",
        help = "Colors new particles pick from"
    )]
    pub palette: Vec<String>,
    #[arg(long, value_name = "H,S,V", value_parser = crate::seeding::three, requires = "palette", help = "Noise from 0 to 1 added to the hue, saturation and value of the palette")]
    pub palette_noise: Option<[f64; 3]>,
    #[arg(long, value_name = "TIME:COLOR", value_parser = crate::sowed::color_stop, conflicts_with_all = ["hsv", "palette"], help = "A stop of the gradient new particles pick from, TIME from 0 to 1; repeat it")]
    pub gradient: Vec<String>,
}

pub(crate) fn three(text: &str) -> Result<[f64; 3], String> {
    let n: Vec<f64> = text.split(',').filter_map(|v| v.trim().parse().ok()).collect();
    <[f64; 3]>::try_from(n).map_err(|_| fill(told("%1 is not three numbers H,S,V"), &[&text]))
}

impl Tint {
    pub(crate) fn fill(self, ranges: &mut Map<String, Value>) {
        if let Some(hue) = self.hsv {
            let mut h = json!({ "hue": hue });
            self.saturation.iter().for_each(|s| h["sat"] = json!(s));
            self.value.iter().for_each(|v| h["val"] = json!(v));
            self.hue_steps.iter().for_each(|n| h["steps"] = json!(n));
            ranges.insert("hsv".into(), h);
        }
        if !self.palette.is_empty() {
            let mut p = json!({ "colors": self.palette });
            self.palette_noise.iter().for_each(|n| p["noise"] = json!(n));
            ranges.insert("palette".into(), p);
        }
        let stop = |s: &String| {
            s.split_once(':').and_then(|(t, c)| Some(json!([t.trim().parse::<f64>().ok()?, c.trim()])))
        };
        let stops: Vec<Value> = self.gradient.iter().filter_map(stop).collect();
        if !stops.is_empty() {
            ranges.insert("gradient".into(), json!(stops));
        }
    }
}

pub(crate) fn edit(system: String, what: Seeding) -> P {
    P::Sow(match what {
        Seeding::Emitter {
            index,
            burst,
            every,
            on,
            periodic,
            duration,
            delay,
            follow,
            audio_gate,
            inherit,
            clear,
        } => {
            let mut values = Map::new();
            if let Some(count) = burst {
                let mut b = json!({ "count": count });
                every.iter().for_each(|e| b["every"] = json!(e));
                on.iter().for_each(|o| b["on"] = json!(o));
                values.insert("burst".into(), b);
            }
            periodic.iter().for_each(|[on, off]| {
                drop(values.insert("periodic".into(), json!({ "on": on, "off": off })))
            });
            duration.iter().for_each(|d| drop(values.insert("duration".into(), json!(d))));
            delay.iter().for_each(|d| drop(values.insert("delay".into(), json!(d))));
            follow.iter().for_each(|f| {
                drop(values.insert("follow".into(), if *f { json!("cursor") } else { Value::Null }))
            });
            audio_gate.iter().for_each(|a| drop(values.insert("audio_gate".into(), json!(a))));
            inherit.iter().for_each(|i| drop(values.insert("inherit".into(), json!(i))));
            S::Emit { system, emitter: index, values, clear }
        }
        Seeding::Window { index, window } => {
            let n: Vec<f64> = window.split(',').filter_map(|v| v.trim().parse().ok()).collect();
            S::Window { system, index, window: <[f64; 4]>::try_from(n).ok() }
        }
        Seeding::Rename { to } => S::Rename { system, to },
    })
}
