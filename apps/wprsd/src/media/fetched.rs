use super::pictures::Pictures;
use image::{ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn png(color: [u8; 4]) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    RgbaImage::from_pixel(8, 8, Rgba(color)).write_to(&mut out, ImageFormat::Png).unwrap();
    out.into_inner()
}

async fn site(body: Vec<u8>) -> (String, Arc<AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}/cover.png", listener.local_addr().unwrap());
    let served = Arc::new(AtomicUsize::new(0));
    let counted = served.clone();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let mut asked = [0u8; 1024];
            let _ = stream.read(&mut asked).await;
            counted.fetch_add(1, Ordering::SeqCst);
            let head =
                format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = stream.write_all(&[head.as_bytes(), &body].concat()).await;
        }
    });
    (address, served)
}

#[tokio::test]
async fn covers_from_the_internet_are_fetched_once_and_dropped_when_turned_off() {
    let (address, served) = site(png([0, 255, 255, 255])).await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("red.png"), png([255, 0, 0, 255])).unwrap();
    let local = reqwest::Url::from_file_path(dir.path().join("red.png")).unwrap().to_string();
    let (sender, mut arrivals) = tokio::sync::mpsc::unbounded_channel();
    let mut p = Pictures::new(sender);
    p.ask(1, &address, true);
    p.arrived(arrivals.recv().await.unwrap());
    let fetched = p.serial();
    assert!(fetched != 0 && served.load(Ordering::SeqCst) == 1);
    p.ask(2, &local, true);
    p.arrived(arrivals.recv().await.unwrap());
    assert_eq!(p.previous.as_ref().map(|c| c.serial), Some(fetched));
    p.ask(3, &address, true);
    assert!(arrivals.try_recv().is_err(), "a cover already fetched is not asked for again");
    assert_eq!((p.serial(), served.load(Ordering::SeqCst)), (fetched, 1));
    p.ask(4, &local, false);
    p.arrived(arrivals.recv().await.unwrap());
    assert!(p.serial() != fetched, "a local cover still shows with the switch off");
    let kept = [p.serial(), p.previous.as_ref().map_or(0, |c| c.serial)];
    assert!(!kept.contains(&fetched), "a cover from the internet goes away when the switch is turned off");
    p.ask(5, &address, false);
    assert_eq!(p.serial(), 0);
    p.ask(6, &address, true);
    p.arrived(arrivals.recv().await.unwrap());
    assert!(p.serial() != 0 && served.load(Ordering::SeqCst) == 2, "turning it off forgets what was fetched");
}
