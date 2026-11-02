use crate::show::{clean, table};
use anyhow::Result;
use clap::Args;
use serde_json::json;
use wprs_client::Client;
use wprs_core::model::State;

#[derive(Args, Debug)]
pub struct GpuArgs {
    #[arg(
        help = "Choose the card: auto, integrated, dedicated, software or part of its name; list the cards without it"
    )]
    pub choice: Option<String>,
    #[arg(long, help = crate::cli::JSON)]
    pub json: bool,
}

pub const LATER: &str =
    "the change takes effect the next time the daemon starts (systemctl --user restart wprsd)";

pub async fn run(client: &Client, args: GpuArgs) -> Result<()> {
    if let Some(choice) = &args.choice {
        client.daemon.set_option("render.gpu", choice).await?;
    }
    let state: State = serde_json::from_str(&client.daemon.get_state().await?)?;
    let chosen = state.options.get("render.gpu").and_then(|v| v.as_str()).unwrap_or("auto").to_owned();
    if args.json {
        let reply = json!({ "gpu": chosen, "adapters": state.adapters, "warning": state.gpu_warning,
            "restart": state.gpu_restart, "tier": state.tier, "tier_measured": state.tier_measured });
        println!("{reply}");
        return Ok(());
    }
    let rows = state.adapters.iter().map(|a| {
        let used = if a.used { "*" } else { "" };
        vec![used.into(), a.name.clone(), a.kind.clone(), a.backend.clone()]
    });
    print!("{}", table(&["USED", "NAME", "KIND", "BACKEND"], rows));
    println!("render.gpu = {}", clean(&chosen));
    if let Some(warning) = &state.gpu_warning {
        println!("warning: {}", clean(warning));
    }
    if state.gpu_restart {
        println!("{LATER}");
    }
    Ok(())
}

pub async fn notice(client: &Client, key: &str) -> Result<()> {
    if key != "render.gpu" {
        return Ok(());
    }
    let state: State = serde_json::from_str(&client.daemon.get_state().await?)?;
    if state.gpu_restart {
        println!("{LATER}");
    }
    Ok(())
}
