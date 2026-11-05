use crate::show::{clean, table};
use anyhow::Result;
use clap::Args;
use serde_json::json;
use wprs_client::Client;
use wprs_core::model::{OutputState, State};
use wprs_core::{HdrChoice, QualityChoice};

const SETTLE: std::time::Duration = std::time::Duration::from_millis(300);

#[derive(Args, Debug)]
pub struct QualityArgs {
    #[arg(
        help = "auto, a fixed level q0 to q3, or min-q1 or min-q2 to keep the automatic choice from going lower; with --output, none follows the general setting; show the levels without it"
    )]
    pub level: Option<String>,
    #[arg(
        short,
        long,
        default_value = "",
        help = "Output name such as DP-1 for its own level; every output when omitted"
    )]
    pub output: String,
    #[arg(long, value_parser = ["auto", "on", "off", "none"], help = HDR)]
    pub hdr: Option<String>,
    #[arg(long, help = crate::cli::JSON)]
    pub json: bool,
}

const HDR: &str = "Show colors brighter than white on HDR screens: auto (on mains power at the highest level), on or off; with --output, none follows the general setting";

fn hdr(o: &OutputState) -> String {
    let h = &o.hdr;
    let peak = h.peak_nits.map(|n| format!(", {n} nits")).unwrap_or_default();
    format!("{} {} ({}{peak})", h.choice, if h.active { "on" } else { "off" }, h.reason)
}

pub fn note(o: &OutputState) -> Option<String> {
    let over = o.quality_budget > 0.0 && o.quality_cost > o.quality_budget;
    let note = match o.quality_auto.as_str() {
        "stop" if !over && o.quality_reason == "user" => TRIANGLES.to_owned(),
        "stop" if !over => STOPPED.to_owned(),
        auto if over => format!(
            "this scene costs {:.1} units a frame at {}, more than the {:.0} this device is estimated to hold: frames may drop and battery use rise; automatic quality would {}",
            o.quality_cost,
            o.quality,
            o.quality_budget,
            if auto == "stop" { "keep it still at its poster" } else { "lower it" }
        ),
        _ => return None,
    };
    let left = o.left_out.len();
    Some(match left {
        0 => note,
        _ => format!(
            "{note}; {left} {} out at every level to protect the graphics card",
            WORDS[usize::from(left > 1)]
        ),
    })
}

const STOPPED: &str = "this scene is too heavy for this device and automatic quality keeps it still at its poster; a fixed level such as wprs-ctl quality q0 plays it anyway, with fewer frames per second and more battery use";
const TRIANGLES: &str = "this scene draws more model triangles than this device holds: frames may drop and battery use rise; automatic quality would keep it still at its poster";
const WORDS: [&str; 2] = ["effect stays", "effects stay"];

pub fn notes(state: &str) {
    let Ok(state) = serde_json::from_str::<State>(state) else {
        return;
    };
    state
        .outputs
        .iter()
        .filter_map(|(name, o)| Some((name, note(o)?)))
        .for_each(|(name, n)| eprintln!("{name}: {n}"));
}

fn general(state: &State) -> String {
    state.options.get(QualityChoice::KEY).and_then(|v| v.as_str()).unwrap_or("auto").to_owned()
}

pub async fn run(client: &Client, args: QualityArgs) -> Result<()> {
    let output = args.output.trim();
    if let Some(level) = &args.level {
        let key = if output.is_empty() {
            QualityChoice::KEY.to_owned()
        } else {
            format!("{}{output}", QualityChoice::OWN)
        };
        client.daemon.set_option(&key, level).await?;
        tokio::time::sleep(SETTLE).await;
    }
    if let Some(choice) = &args.hdr {
        let key =
            if output.is_empty() { HdrChoice::KEY.to_owned() } else { format!("{}{output}", HdrChoice::OWN) };
        client.daemon.set_option(&key, choice).await?;
        tokio::time::sleep(SETTLE).await;
    }
    let state: State = serde_json::from_str(&client.daemon.get_state().await?)?;
    let shown = state.outputs.iter().filter(|(name, _)| output.is_empty() || name.as_str() == output);
    if args.json {
        let rows = shown.map(|(name, o)| {
            json!({ "output": name, "choice": o.quality_choice, "quality": o.quality, "reason": o.quality_reason,
                "cost": o.quality_cost, "budget": o.quality_budget, "note": note(o), "hdr": o.hdr })
        });
        let own = &state.qualities;
        println!(
            "{}",
            json!({ "quality": general(&state), "own": own, "hdr": state.options.get(HdrChoice::KEY),
                "own_hdr": state.hdrs, "outputs": rows.collect::<Vec<_>>() })
        );
        return Ok(());
    }
    let rows: Vec<_> = shown.collect();
    let cells = rows.iter().map(|(name, o)| {
        let chosen = o.quality_choice.clone().unwrap_or_else(|| general(&state));
        vec![name.to_string(), chosen, o.quality.clone(), o.quality_reason.clone(), hdr(o)]
    });
    print!("{}", table(&["OUTPUT", "CHOICE", "LEVEL", "WHY", "HDR"], cells));
    println!("{} = {}", QualityChoice::KEY, clean(&general(&state)));
    if !output.is_empty() && !state.outputs.contains_key(output) {
        let own = state.qualities.get(output).map_or("none", String::as_str);
        println!("{}{output} = {}, not connected", QualityChoice::OWN, clean(own));
    }
    rows.iter().filter_map(|(name, o)| Some((name, note(o)?))).for_each(|(name, n)| println!("{name}: {n}"));
    Ok(())
}

pub fn own(state: &serde_json::Value, key: &str, json: bool) -> bool {
    let quality = key.strip_prefix(QualityChoice::OWN).map(|o| ("qualities", o));
    let Some((list, output)) = quality.or_else(|| key.strip_prefix(HdrChoice::OWN).map(|o| ("hdrs", o)))
    else {
        return false;
    };
    let own = state[list].get(output).cloned().unwrap_or_default();
    match json {
        true => println!("{}", serde_json::Value::Object([(key.to_owned(), own)].into_iter().collect())),
        false => println!("{key} = {}", own.as_str().unwrap_or("none")),
    }
    true
}

pub async fn notice(client: &Client, key: &str) -> Result<()> {
    if !key.starts_with(QualityChoice::KEY) {
        return Ok(());
    }
    tokio::time::sleep(SETTLE).await;
    let state: State = serde_json::from_str(&client.daemon.get_state().await?)?;
    state
        .outputs
        .iter()
        .filter_map(|(name, o)| Some((name, note(o)?)))
        .for_each(|(name, n)| println!("{name}: {n}"));
    Ok(())
}

#[cfg(test)]
mod tests;
