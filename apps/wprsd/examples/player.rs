use serde_json::Value;
use std::collections::HashMap;
use std::io::BufRead;
use zbus::object_server::SignalEmitter;
use zbus::{OwnedVariant, Variant};

const PATH: &str = "/org/mpris/MediaPlayer2";

#[derive(Default)]
struct Player {
    status: String,
    metadata: HashMap<String, OwnedVariant>,
    position: i64,
    rate: f64,
    gets: std::sync::atomic::AtomicU32,
}

struct Root;

#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    #[zbus(property)]
    fn identity(&self) -> String {
        "Example Player".into()
    }
}

#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    #[zbus(property)]
    fn playback_status(&self) -> String {
        self.status.clone()
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedVariant> {
        self.metadata.iter().map(|(k, v)| (k.clone(), v.try_clone().expect("plain values"))).collect()
    }

    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> i64 {
        self.gets.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.position
    }

    #[zbus(property)]
    fn rate(&self) -> f64 {
        self.rate
    }

    #[zbus(signal)]
    async fn seeked(emitter: &SignalEmitter<'_>, position: i64) -> zbus::Result<()>;
}

fn owned(v: Variant<'_>) -> OwnedVariant {
    v.try_to_owned().expect("plain values")
}

fn track(line: &Value) -> HashMap<String, OwnedVariant> {
    let text = |k: &str| line[k].as_str().unwrap_or_default().to_owned();
    let list =
        |k: &str| line[k].as_array().into_iter().flatten().filter_map(|v| v.as_str()).map(String::from);
    let mut data = HashMap::new();
    data.insert(
        "mpris:trackid".into(),
        owned(Variant::from(zbus::ObjectPath::try_from("/example/track").unwrap())),
    );
    data.insert("xesam:title".into(), owned(Variant::from(text("title"))));
    data.insert("xesam:artist".into(), owned(Variant::from(list("artist").collect::<Vec<_>>())));
    data.insert("xesam:album".into(), owned(Variant::from(text("album"))));
    data.insert("xesam:albumArtist".into(), owned(Variant::from(list("album_artist").collect::<Vec<_>>())));
    data.insert("xesam:genre".into(), owned(Variant::from(list("genre").collect::<Vec<_>>())));
    data.insert(
        "mpris:length".into(),
        owned(Variant::from((line["length"].as_f64().unwrap_or(0.0) * 1e6) as i64)),
    );
    if let Some(art) = line["art"].as_str() {
        data.insert("mpris:artUrl".into(), owned(Variant::from(art.to_owned())));
    }
    data
}

async fn apply(bus: &zbus::Connection, line: &Value) -> zbus::Result<()> {
    let shown = bus.object_server().interface::<_, Player>(PATH).await?;
    let emitter = shown.signal_emitter();
    let mut player = shown.get_mut().await;
    if let Some(status) = line["status"].as_str() {
        player.status = status.into();
        player.playback_status_changed(emitter).await?;
    }
    if line.get("title").is_some() {
        player.metadata = track(line);
        player.metadata_changed(emitter).await?;
    }
    if let Some(rate) = line["rate"].as_f64() {
        player.rate = rate;
        player.rate_changed(emitter).await?;
    }
    if let Some(at) = line["position"].as_f64() {
        player.position = (at * 1e6) as i64;
    }
    if let Some(at) = line["seek"].as_f64() {
        player.position = (at * 1e6) as i64;
        Player::seeked(emitter, player.position).await?;
    }
    if line["gets"].as_bool() == Some(true) {
        println!("gets {}", player.gets.load(std::sync::atomic::Ordering::Relaxed));
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> zbus::Result<()> {
    let name = std::env::args().nth(1).unwrap_or_else(|| "example".into());
    let player = Player { status: "Stopped".into(), rate: 1.0, ..Player::default() };
    let builder = zbus::connection::Builder::session().name(format!("org.mpris.MediaPlayer2.{name}"));
    let bus = builder.serve_at(PATH, Root).serve_at(PATH, player).build().await?;
    let (tx, mut lines) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        std::io::stdin().lock().lines().map_while(Result::ok).try_for_each(|l| tx.send(l))
    });
    println!("ready");
    while let Some(line) = lines.recv().await {
        match serde_json::from_str::<Value>(&line) {
            Ok(line) if line["quit"] == true => break,
            Ok(line) => apply(&bus, &line).await?,
            Err(e) => eprintln!("not JSON: {e}"),
        }
        println!("done");
    }
    Ok(())
}
