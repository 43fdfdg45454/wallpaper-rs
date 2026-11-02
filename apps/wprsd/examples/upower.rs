use std::io::BufRead;

struct Power(bool);

#[zbus::interface(name = "org.freedesktop.UPower")]
impl Power {
    #[zbus(property)]
    fn on_battery(&self) -> bool {
        self.0
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> zbus::Result<()> {
    let path = "/org/freedesktop/UPower";
    let builder = zbus::connection::Builder::system().name("org.freedesktop.UPower");
    let bus = builder.serve_at(path, Power(true)).build().await?;
    let (tx, mut lines) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        std::io::stdin().lock().lines().map_while(Result::ok).try_for_each(|l| tx.send(l))
    });
    println!("ready");
    while let Some(line) = lines.recv().await {
        let shown = bus.object_server().interface::<_, Power>(path).await?;
        shown.get_mut().await.0 = line.trim() == "battery";
        shown.get().await.on_battery_changed(shown.signal_emitter()).await?;
    }
    Ok(())
}
