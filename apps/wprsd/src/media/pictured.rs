use super::covers::{MAX_FILE, MAX_SIDE, decode, fetch, local, reduced};
use super::pictures::Pictures;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use reqwest::Url;
use std::io::Cursor;
use std::time::Duration;

fn png(w: u32, h: u32, color: [u8; 4]) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    RgbaImage::from_pixel(w, h, Rgba(color)).write_to(&mut out, ImageFormat::Png).unwrap();
    out.into_inner()
}

#[test]
fn covers_shrink_to_256_pixels_and_keep_their_shape() {
    assert_eq!(reduced(DynamicImage::new_rgba8(600, 300)).dimensions(), (256, 128));
    assert_eq!(reduced(DynamicImage::new_rgb8(90, 2000)).dimensions(), (12, 256));
    assert_eq!(reduced(DynamicImage::new_rgba8(64, 64)).dimensions(), (64, 64));
    let cover = decode(&png(512, 512, [200, 10, 10, 255])).unwrap();
    assert_eq!(cover.pixels.size(), (256, 256));
    assert!(decode(b"not a picture").is_none());
    assert!(decode(&png(MAX_SIDE, 2, [9, 9, 9, 255])).is_some());
    assert!(
        decode(&png(MAX_SIDE + 1, 2, [9, 9, 9, 255])).is_none(),
        "a cover wider than 4096 pixels is refused"
    );
    assert!(decode(&png(4, 4, [0; 4])).unwrap().serial < decode(&png(4, 4, [0; 4])).unwrap().serial);
}

#[test]
fn only_local_files_of_at_most_16_mib_are_read_as_covers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cover art.png");
    std::fs::write(&path, png(8, 8, [1, 2, 3, 255])).unwrap();
    let url = Url::from_file_path(&path).unwrap();
    assert!(url.as_str().contains("cover%20art.png"));
    assert!(local(&url).is_some_and(|b| b.len() < 1024));
    assert!(local(&Url::from_file_path(dir.path()).unwrap()).is_none(), "a folder is no cover");
    let big = dir.path().join("big.png");
    std::fs::File::create(&big).unwrap().set_len(MAX_FILE + 1).unwrap();
    assert!(local(&Url::from_file_path(&big).unwrap()).is_none());
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        assert!(fetch(url.as_str(), false).await.is_some());
        let refused = tokio::time::timeout(Duration::from_secs(1), fetch("https://example.org/a.png", false));
        assert!(refused.await.unwrap().is_none(), "covers from the internet need media.remote_covers");
        assert!(fetch("ftp://example.org/a.png", true).await.is_none());
        assert!(fetch("data:image/png;base64,AAAA", true).await.is_none());
    });
}

#[test]
fn a_new_cover_pushes_the_last_one_to_previous() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let file = |name: &str, c: [u8; 4]| {
            let path = dir.path().join(name);
            std::fs::write(&path, png(8, 8, c)).unwrap();
            Url::from_file_path(path).unwrap().to_string()
        };
        let (red, blue) = (file("red.png", [255, 0, 0, 255]), file("blue.png", [0, 0, 255, 255]));
        let (sender, mut arrivals) = tokio::sync::mpsc::unbounded_channel();
        let mut p = Pictures::new(sender);
        p.ask(1, &red, false);
        p.arrived(arrivals.recv().await.unwrap());
        let first = p.serial();
        assert!(first != 0 && p.previous.is_none());
        p.ask(1, &red, false);
        assert!(arrivals.try_recv().is_err(), "the same track is not read again");
        p.ask(2, &red, false);
        p.arrived(arrivals.recv().await.unwrap());
        assert_eq!((p.serial(), p.previous.is_none()), (first, true), "an unchanged file keeps its cover");
        std::fs::write(Url::parse(&red).unwrap().to_file_path().unwrap(), png(8, 8, [0, 255, 0, 255]))
            .unwrap();
        p.ask(3, &red, false);
        p.arrived(arrivals.recv().await.unwrap());
        let rewritten = p.serial();
        assert!(rewritten != first, "a file rewritten for a new track is read again");
        assert_eq!(p.previous.as_ref().map(|c| c.serial), Some(first));
        p.ask(4, &blue, false);
        p.arrived(arrivals.recv().await.unwrap());
        assert_eq!(p.previous.as_ref().map(|c| c.serial), Some(rewritten));
        p.ask(5, "", false);
        assert_eq!((p.serial(), p.previous.is_some()), (0, true));
        p.ask(6, "https://example.org/cover.png", false);
        assert_eq!(p.serial(), 0, "a cover from the internet stays out while the switch is off");
    });
}
