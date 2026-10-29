use std::io::BufRead;

struct Timedate(String);

#[zbus::interface(name = "org.freedesktop.timedate1")]
impl Timedate {
    #[zbus(property)]
    fn timezone(&self) -> String {
        self.0.clone()
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> zbus::Result<()> {
    let path = "/org/freedesktop/timedate1";
    let builder = zbus::connection::Builder::system().name("org.freedesktop.timedate1");
    let bus = builder.serve_at(path, Timedate("Etc/UTC".into())).build().await?;
    let (tx, mut zones) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        std::io::stdin().lock().lines().map_while(Result::ok).try_for_each(|z| tx.send(z))
    });
    println!("ready");
    while let Some(zone) = zones.recv().await {
        let shown = bus.object_server().interface::<_, Timedate>(path).await?;
        shown.get_mut().await.0 = zone.trim().into();
        shown.get().await.timezone_changed(shown.signal_emitter()).await?;
    }
    Ok(())
}
