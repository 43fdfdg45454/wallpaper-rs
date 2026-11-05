use crate::cli::JSON;
use crate::show::list;
use anyhow::Result;
use clap::Subcommand;
use wprs_client::{Client, parse};
use wprs_core::model::{InputsReply, MediaStatus};

#[derive(Subcommand, Debug)]
pub enum Media {
    #[command(
        about = "Show whether the media player is read and for which screens; track data is never shown here"
    )]
    Status {
        #[arg(long, help = JSON)]
        json: bool,
    },
    #[command(
        about = "Show a made-up track on the wallpaper of a screen instead of what a media player plays, to try media values, triggers and scripts; --stop ends it"
    )]
    Simulate {
        #[arg(help = "The screen")]
        output: String,
        #[arg(required_unless_present = "stop", help = MEDIA)]
        file: Option<std::path::PathBuf>,
        #[arg(long, conflicts_with = "file")]
        stop: bool,
    },
}

fn status(s: &MediaStatus) -> String {
    let yes = |on: bool| if on { "yes" } else { "no" };
    let mut lines = vec![match s.listening {
        true => format!("media player: read for {}", list(s.using.iter())),
        false => "media player: not read (no shown wallpaper allowed to use it needs it)".to_owned(),
    }];
    if s.listening {
        lines.push(format!("players: {}", s.players));
        lines.push(format!("track: {}, playing: {}, cover: {}", yes(s.track), yes(s.playing), yes(s.cover)));
    }
    let remote = if s.remote_covers { "on" } else { "off" };
    lines.push(format!("covers from the internet (media.remote_covers): {remote}"));
    lines.join("\n") + "\n"
}

pub async fn run(client: &Client, command: Media) -> Result<()> {
    let json = match command {
        Media::Status { json } => json,
        Media::Simulate { output, file: None, .. } => {
            return Ok(client.inputs.simulate_media(&output, "", &[]).await?);
        }
        Media::Simulate { output, file: Some(file), .. } => {
            let mut track = simulated(&file)?;
            let cover = match track.as_object_mut().and_then(|t| t.remove("cover")) {
                Some(serde_json::Value::String(path)) if !path.is_empty() => {
                    let path = file.parent().unwrap_or(std::path::Path::new(".")).join(path);
                    std::fs::read(&path)
                        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))?
                }
                _ => Vec::new(),
            };
            return Ok(client.inputs.simulate_media(&output, &track.to_string(), &cover).await?);
        }
    };
    let reply: InputsReply = parse(&client.inputs.get_inputs().await?)?;
    match json {
        true => println!("{}", serde_json::to_string(&reply.media)?),
        false => print!("{}", status(&reply.media)),
    }
    Ok(())
}

pub const MEDIA: &str = "JSON file of a made-up track shown instead of a media player: title, artist, album, album_artist, subtitle, genres, playing, position, duration, rate and cover (a picture of the folder)";

pub fn simulated(path: &std::path::Path) -> Result<serde_json::Value> {
    let bytes = std::fs::read(path).map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))?;
    anyhow::ensure!(bytes.len() <= 64 << 10, "{} is larger than 64 KiB", path.display());
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    anyhow::ensure!(value.is_object(), "{} holds a JSON object of the made-up track", path.display());
    Ok(value)
}
