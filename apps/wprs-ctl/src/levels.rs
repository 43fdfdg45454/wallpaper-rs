use crate::show::{clean, table};
use anyhow::Result;
use clap::Args;
use serde_json::json;
use wprs_client::Client;
use wprs_core::QualityChoice;
use wprs_core::model::{OutputState, State};

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
    #[arg(long, help = crate::cli::JSON)]
    pub json: bool,
}

pub fn note(o: &OutputState) -> Option<String> {
    (o.quality_reason == "user" && o.quality_cost > o.quality_budget && o.quality_budget > 0.0).then(|| {
        format!(
            "this scene costs {:.1} units a frame at {}, more than the {:.0} this graphics card is estimated to hold; expect fewer frames per second",
            o.quality_cost, o.quality, o.quality_budget
        )
    })
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
    let state: State = serde_json::from_str(&client.daemon.get_state().await?)?;
    let shown = state.outputs.iter().filter(|(name, _)| output.is_empty() || name.as_str() == output);
    if args.json {
        let rows = shown.map(|(name, o)| {
            json!({ "output": name, "choice": o.quality_choice, "quality": o.quality, "reason": o.quality_reason,
                "cost": o.quality_cost, "budget": o.quality_budget, "note": note(o) })
        });
        let own = &state.qualities;
        println!(
            "{}",
            json!({ "quality": general(&state), "own": own, "outputs": rows.collect::<Vec<_>>() })
        );
        return Ok(());
    }
    let rows: Vec<_> = shown.collect();
    let cells = rows.iter().map(|(name, o)| {
        let chosen = o.quality_choice.clone().unwrap_or_else(|| general(&state));
        vec![name.to_string(), chosen, o.quality.clone(), o.quality_reason.clone()]
    });
    print!("{}", table(&["OUTPUT", "CHOICE", "LEVEL", "WHY"], cells));
    println!("{} = {}", QualityChoice::KEY, clean(&general(&state)));
    if !output.is_empty() && !state.outputs.contains_key(output) {
        let own = state.qualities.get(output).map_or("none", String::as_str);
        println!("{}{output} = {}, not connected", QualityChoice::OWN, clean(own));
    }
    rows.iter().filter_map(|(name, o)| Some((name, note(o)?))).for_each(|(name, n)| println!("{name}: {n}"));
    Ok(())
}

pub fn own(state: &serde_json::Value, key: &str, json: bool) -> bool {
    let Some(output) = key.strip_prefix(QualityChoice::OWN) else { return false };
    let own = state["qualities"].get(output).cloned().unwrap_or_default();
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
