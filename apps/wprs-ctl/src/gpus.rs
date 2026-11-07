use crate::show::{clean, table};
use anyhow::Result;
use clap::Args;
use futures_util::StreamExt;
use serde_json::json;
use wprs_client::Client;
use wprs_core::LayerNamespace;
use wprs_core::model::{LayerShellState, State};

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

pub async fn restart(client: &Client) -> Result<()> {
    let bus = zbus::fdo::DBusProxy::new(&client.bus).await?;
    let mut owners = bus.receive_name_owner_changed_with_args(&[(0, wprs_core::dbus::BUS_NAME)]).await?;
    let name = wprs_core::dbus::BUS_NAME.try_into()?;
    let before = bus.get_name_owner(name).await.ok().map(|owner| owner.to_string());
    client.daemon.restart().await?;
    let back = async {
        while let Some(signal) = owners.next().await {
            let Ok(args) = signal.args() else { continue };
            let now = args.new_owner.as_ref().map(ToString::to_string);
            if now.is_some() && now != before {
                return true;
            }
        }
        false
    };
    let started = tokio::time::timeout(std::time::Duration::from_secs(30), back).await;
    anyhow::ensure!(matches!(started, Ok(true)), "the daemon did not start again within 30 seconds");
    let state: State = serde_json::from_str(&client.daemon.get_state().await?)?;
    println!("the daemon started again on {}", clean(&state.adapter));
    Ok(())
}

pub async fn run(client: &Client, args: GpuArgs) -> Result<()> {
    if let Some(choice) = &args.choice {
        client.daemon.set_option("render.gpu", choice).await?;
    }
    let state: State = serde_json::from_str(&client.daemon.get_state().await?)?;
    let chosen = state.options.get("render.gpu").and_then(|v| v.as_str()).unwrap_or("auto").to_owned();
    if args.json {
        let reply = json!({ "gpu": chosen, "adapters": state.adapters, "warning": state.gpu_warning,
            "note": state.gpu_note, "restart": state.gpu_restart, "tier": state.tier, "tier_measured": state.tier_measured });
        println!("{reply}");
        return Ok(());
    }
    let rows = state.adapters.iter().map(|a| {
        let used = if a.used { "*" } else { "" };
        let bus = if a.bus.is_empty() { "-".into() } else { format!("pci:{}", a.bus) };
        let desktop = if a.compositor { "*" } else { "" };
        vec![used.into(), desktop.into(), a.name.clone(), a.kind.clone(), a.backend.clone(), bus]
    });
    print!("{}", table(&["USED", "DESKTOP", "NAME", "KIND", "BACKEND", "BUS"], rows));
    println!("render.gpu = {}", clean(&chosen));
    if let Some(warning) = &state.gpu_warning {
        println!("warning: {}", clean(warning));
    }
    if let Some(note) = &state.gpu_note {
        println!("note: {}", clean(note));
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
    if key == "render.gpu" && state.gpu_restart {
        println!("{LATER}");
    }
    if key == LayerNamespace::KEY {
        in_use(&state.layer_shell);
    }
    Ok(())
}

pub fn in_use(layer: &LayerShellState) {
    if layer.restart {
        println!("{LATER}");
    }
    if let Some(namespace) = layer.namespace.as_deref() {
        println!("in use: {} (from {})", clean(namespace), clean(&layer.from));
        if let Some(ignored) = &layer.ignored {
            println!("warning: {}", clean(ignored));
        }
    }
}
