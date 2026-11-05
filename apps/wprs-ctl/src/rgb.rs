use crate::cli::JSON;
use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use wprs_client::Client;
use wprs_core::rgb::Covered;

const COVERED: &str = "follow keeps the lights following the scene without drawing it, pause sends nothing and leaves the lights as they are until the wallpaper shows again";

fn covered(s: &Value) -> String {
    let choice = s["when_covered"].as_str().unwrap_or("pause");
    let now = if s["held"] == true { ", paused now" } else { "" };
    format!("while covered ({}): {choice}{now}", Covered::KEY)
}

#[derive(Subcommand, Debug)]
pub enum Rgb {
    #[command(
        about = "Show whether RGB lighting is on, its OpenRGB server, the connection and what sends colors"
    )]
    Status {
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Connect to the OpenRGB server now and list its devices and lights")]
    Devices {
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Show a moving rainbow for 3 seconds on one device, or on all of them")]
    Test {
        #[arg(help = "Index from wprs-ctl rgb devices; every device when omitted")]
        device: Option<u32>,
    },
    #[command(
        about = "Show or choose what the lights do while a window covers the wallpaper: follow it or pause"
    )]
    Covered {
        #[arg(value_parser = Covered::WORDS, help = COVERED)]
        choice: Option<String>,
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(about = "Print the colors a screen sends to RGB lighting, at most 64 by 16")]
    Strip {
        #[arg(short, long, help = crate::cli::OUTPUT)]
        output: String,
        #[arg(long, help = JSON)]
        json: bool,
    },
}

fn status(s: &Value) -> String {
    let on = if s["enabled"] == true { "on" } else { "off" };
    let place = if s["local"] == true { "this computer" } else { "another computer on the network" };
    let mut lines =
        vec![format!("RGB lighting: {on}, OpenRGB server {} ({place})", s["server"].as_str().unwrap_or("?"))];
    lines.push(match (s["connected"] == true, s["error"].as_str()) {
        (true, _) => format!("connected, {} devices", s["devices"].as_array().map_or(0, Vec::len)),
        (false, Some(e)) => format!("not connected: {e}"),
        (false, None) => "not connected".into(),
    });
    lines.push(covered(s));
    if let Some(source) = s["source"].as_str() {
        lines.push(format!("colors from {source}, {} frames sent", s["frames"]));
    }
    lines.join("\n")
}

fn devices(s: &Value) -> String {
    let rows = s["devices"].as_array().into_iter().flatten().map(|d| {
        let kind = d["kind"].as_str().unwrap_or("?");
        format!("{:>3}  {kind:<14} {}  ({} lights)", d["index"], d["name"].as_str().unwrap_or("?"), d["leds"])
    });
    let list: Vec<String> = rows.collect();
    if list.is_empty() { "no devices".into() } else { list.join("\n") }
}

pub async fn run(client: &Client, command: Rgb) -> Result<()> {
    let rgb = &client.rgb;
    match command {
        Rgb::Status { json } => {
            let s: Value = serde_json::from_str(&rgb.get_status().await?)?;
            println!("{}", if json { s.to_string() } else { status(&s) });
        }
        Rgb::Devices { json } => {
            rgb.connect().await?;
            let s: Value = serde_json::from_str(&rgb.get_status().await?)?;
            println!("{}", if json { s["devices"].to_string() } else { devices(&s) });
        }
        Rgb::Test { device } => rgb.test(device.map_or(-1, |d| d as i32)).await?,
        Rgb::Covered { choice, json } => {
            if let Some(choice) = choice {
                client.daemon.set_option(Covered::KEY, &choice).await?;
            }
            let s: Value = serde_json::from_str(&rgb.get_status().await?)?;
            let shown = json!({ "when_covered": s["when_covered"], "held": s["held"] });
            println!("{}", if json { shown.to_string() } else { covered(&s) });
        }
        Rgb::Strip { output, json } => {
            let (w, h, colors) = rgb.get_strip(&output).await?;
            let hex: Vec<String> =
                colors.chunks(3).map(|c| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])).collect();
            match json {
                true => println!("{}", json!({ "width": w, "height": h, "colors": hex })),
                false => hex.chunks(w.max(1) as usize).for_each(|row| println!("{}", row.join(" "))),
            }
        }
    }
    Ok(())
}
