use crate::show::{clean, table};
use anyhow::Result;
use clap::Args;
use serde_json::json;
use wprs_client::Client;
use wprs_core::LayerNamespace;
use wprs_core::model::State;

#[derive(Args, Debug)]
pub struct GpuArgs {
    #[arg(
        help = "Choose the card: auto, integrated, dedicated, software, part of its name or pci: and its bus id to tell identical cards apart; list the cards without it"
    )]
    pub choice: Option<String>,
    #[arg(long, help = crate::cli::JSON)]
    pub json: bool,
}

pub const LATER: &str = "the change takes effect the next time the daemon starts (wprs-ctl restart)";

async fn owner(client: &Client) -> Option<String> {
    let bus = zbus::fdo::DBusProxy::new(&client.bus).await.ok()?;
    let name = wprs_core::dbus::BUS_NAME.try_into().ok()?;
    bus.get_name_owner(name).await.ok().map(|owner| owner.to_string())
}

pub async fn restart(client: &Client) -> Result<()> {
    let before = owner(client).await;
    client.daemon.restart().await?;
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let now = owner(client).await;
        if now.is_none() || now == before {
            continue;
        }
        let Ok(text) = client.daemon.get_state().await else { continue };
        let state: State = serde_json::from_str(&text)?;
        println!("the daemon started again on {}", clean(&state.adapter));
        return Ok(());
    }
    anyhow::bail!("the daemon did not start again within 30 seconds")
}

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
        let bus = if a.bus.is_empty() { "-".into() } else { format!("pci:{}", a.bus) };
        vec![used.into(), a.name.clone(), a.kind.clone(), a.backend.clone(), bus]
    });
    print!("{}", table(&["USED", "NAME", "KIND", "BACKEND", "BUS"], rows));
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
    if key != "render.gpu" && key != LayerNamespace::KEY {
        return Ok(());
    }
    let state: State = serde_json::from_str(&client.daemon.get_state().await?)?;
    let layer = &state.layer_shell;
    if (key == "render.gpu" && state.gpu_restart) || (key == LayerNamespace::KEY && layer.restart) {
        println!("{LATER}");
    }
    if let Some(namespace) = layer.namespace.as_deref().filter(|_| key == LayerNamespace::KEY) {
        println!("in use: {} (from {})", clean(namespace), clean(&layer.from));
        if let Some(ignored) = &layer.ignored {
            println!("warning: {}", clean(ignored));
        }
    }
    Ok(())
}
