use crate::decode::{Picture, Problem, decode, package, video};
use image::ImageReader;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::os::fd::FromRawFd;
use std::path::PathBuf;
use wprs_library::{Kind, Meta};

#[derive(Serialize, Deserialize)]
pub struct Request {
    pub path: PathBuf,
    pub kind: Kind,
    pub thumbs: Vec<(u32, PathBuf)>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Response {
    pub meta: Meta,
    pub written: Vec<u32>,
    pub failure: Option<String>,
    pub missing: bool,
    #[serde(skip)]
    pub retry: bool,
}

pub fn run() -> anyhow::Result<()> {
    let replies = unsafe {
        libc::setpriority(libc::PRIO_PROCESS, 0, 19);
        libc::syscall(libc::SYS_ioprio_set, 1, 0, 3 << 13);
        let replies = libc::fcntl(1, libc::F_DUPFD_CLOEXEC, 3);
        anyhow::ensure!(replies >= 0, "cannot set up the reply channel");
        libc::dup2(2, 1);
        std::fs::File::from_raw_fd(replies)
    };
    let mut out = std::io::BufWriter::new(replies);
    for line in std::io::stdin().lock().lines() {
        let request: Request = serde_json::from_str(&line?)?;
        serde_json::to_writer(&mut out, &handle(&request))?;
        out.write_all(b"\n")?;
        out.flush()?;
    }
    Ok(())
}

fn handle(request: &Request) -> Response {
    let edge = request.thumbs.iter().map(|(size, _)| *size).max().unwrap_or(256);
    let picture = match request.kind {
        Kind::Image => match wprs_render::decode_reduced(&request.path, edge) {
            Some((image, (width, height))) => {
                let meta = Meta { width: Some(width), height: Some(height), ..Meta::default() };
                Ok(Picture { meta, image: Some(image), failure: None })
            }
            None => ImageReader::open(&request.path).map_err(Problem::from).and_then(decode).map(Picture::of),
        },
        Kind::Video => video(&request.path),
        Kind::Package => package(&request.path),
    };
    match picture {
        Err(Problem::Missing(e)) => Response { missing: true, failure: Some(e), ..Response::default() },
        Err(Problem::Decode(e)) => {
            Response { meta: Meta { error: Some(e), ..Meta::default() }, ..Response::default() }
        }
        Ok(Picture { mut meta, image, failure }) => {
            let mut written = Vec::new();
            if let Some(image) = image {
                meta.color = Some(wprs_render::palette(&image).colors.first().copied().unwrap_or_default());
                if let Err(e) = wprs_render::thumbnails(image, &request.thumbs, &mut written) {
                    return Response { meta, written, failure: Some(e.to_string()), ..Response::default() };
                }
            }
            Response { meta, written, failure, ..Response::default() }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;
    use std::path::Path;

    #[test]
    fn images_get_meta_color_and_thumbnails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wide.png");
        RgbaImage::from_pixel(600, 300, image::Rgba([0, 255, 0, 255])).save(&path).unwrap();
        let thumbs = vec![(256, dir.path().join("256.jpg")), (1024, dir.path().join("1024.jpg"))];
        let response = handle(&Request { path, kind: Kind::Image, thumbs });
        assert_eq!(
            (response.meta.width, response.meta.height, response.meta.error),
            (Some(600), Some(300), None)
        );
        assert_eq!((response.written, response.failure), (vec![1024, 256], None));
        let small = image::open(dir.path().join("256.jpg")).unwrap();
        assert_eq!((small.width(), small.height()), (256, 128));
        assert_eq!(image::open(dir.path().join("1024.jpg")).unwrap().width(), 600);
        let missing =
            handle(&Request { path: dir.path().join("gone.png"), kind: Kind::Image, thumbs: vec![] });
        assert!(missing.missing && missing.meta.error.is_none());
        let junk = dir.path().join("junk.png");
        std::fs::write(&junk, b"not an image").unwrap();
        let junk = handle(&Request { path: junk, kind: Kind::Image, thumbs: vec![] });
        assert!(!junk.missing && junk.meta.error.is_some());
        let jpeg = dir.path().join("large.jpg");
        image::RgbImage::from_pixel(2400, 1200, image::Rgb([0, 255, 0])).save(&jpeg).unwrap();
        let thumbs = vec![(256, dir.path().join("j256.jpg"))];
        let large = handle(&Request { path: jpeg, kind: Kind::Image, thumbs });
        assert_eq!((large.meta.width, large.meta.height, large.written), (Some(2400), Some(1200), vec![256]));
        let thumb = image::open(dir.path().join("j256.jpg")).unwrap();
        assert_eq!((thumb.width(), thumb.height()), (256, 128));
        for (frame, side) in [(0xc2, 60000u16), (0xc0, 30000)] {
            let mut bytes = std::fs::read(dir.path().join("j256.jpg")).unwrap();
            let at = bytes.windows(2).position(|m| m == [0xff, 0xc0]).unwrap();
            bytes[at + 1] = frame;
            bytes[at + 5..at + 9].copy_from_slice(&[side.to_be_bytes(), side.to_be_bytes()].concat());
            let huge = dir.path().join("huge.jpg");
            std::fs::write(&huge, bytes).unwrap();
            let start = std::time::Instant::now();
            let thumbs = vec![(256, dir.path().join("h.jpg"))];
            let huge = handle(&Request { path: huge, kind: Kind::Image, thumbs });
            assert!(huge.meta.error.is_some() && huge.written.is_empty(), "a {side} px JPEG was decoded");
            assert!(start.elapsed() < std::time::Duration::from_secs(2), "{:?}", start.elapsed());
        }
    }

    fn package(dir: &Path, kind: &str, entry: &str, preview: Option<&str>) -> PathBuf {
        let source = dir.join(kind);
        std::fs::create_dir(&source).unwrap();
        RgbaImage::from_pixel(80, 40, image::Rgba([255, 0, 0, 255])).save(source.join("image.png")).unwrap();
        RgbaImage::from_pixel(20, 20, image::Rgba([0, 0, 255, 255]))
            .save(source.join("preview.png"))
            .unwrap();
        if kind == "scene" {
            let scene = r##"{"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32]}, "layers": [{"id": "fill", "type": "solid", "color": "#ff0000"}]}"##;
            std::fs::write(source.join("scene.json"), scene).unwrap();
        }
        let manifest = serde_json::json!({"schema": wprs_format::SCHEMA_V1, "id": "org.example.calm", "version": "1.0.0",
            "title": "Calm", "license": "CC0-1.0", "kind": kind, "entry": entry, "preview": preview});
        std::fs::write(source.join("manifest.json"), manifest.to_string()).unwrap();
        let wpak = dir.join(format!("{kind}.wpak"));
        wprs_format::build(&source, &wpak, &wprs_format::Limits::default()).unwrap();
        wpak
    }

    #[test]
    fn packages_show_their_preview_and_entry() {
        let dir = tempfile::tempdir().unwrap();
        let wpak = package(dir.path(), "image", "image.png", Some("preview.png"));
        let thumbs = vec![(256, dir.path().join("thumb.jpg"))];
        let response = handle(&Request { path: wpak.clone(), kind: Kind::Package, thumbs });
        assert_eq!(
            (response.meta.width, response.meta.height, response.meta.color),
            (Some(80), Some(40), Some(0xff))
        );
        assert_eq!(image::open(dir.path().join("thumb.jpg")).unwrap().width(), 20);
        let blob = dir.path().join("blobs").join("entry");
        assert!(!crate::apply::materialize(&wpak, &blob).unwrap());
        assert_eq!(
            ImageReader::open(&blob).unwrap().with_guessed_format().unwrap().decode().unwrap().width(),
            80
        );
        let scene = package(dir.path(), "scene", "scene.json", None);
        let response = handle(&Request { path: scene.clone(), kind: Kind::Package, thumbs: vec![] });
        assert_eq!((response.meta.error.as_deref(), response.failure.as_deref()), (None, Some("no preview")));
        assert!(crate::apply::materialize(&scene, &blob.with_file_name("scene")).is_err());
        std::fs::create_dir(dir.path().join("second")).unwrap();
        let shown = package(&dir.path().join("second"), "scene", "scene.json", Some("preview.png"));
        let response = handle(&Request { path: shown, kind: Kind::Package, thumbs: vec![] });
        assert_eq!((response.meta.error, response.failure, response.meta.width), (None, None, Some(64)));
    }

    mod table {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/features.rs"));
    }

    #[test]
    fn packages_the_desktop_cannot_draw_keep_their_preview_and_say_why() {
        let dir = tempfile::tempdir().unwrap();
        let mut cases = table::cases();
        cases.push(("effects".into(), table::effects()));
        for (name, case) in &cases {
            let wpak = table::build(name, case, dir.path()).unwrap_or_else(|e| panic!("{name}: {e}"));
            let facts = wprs_format::open(&wpak, &wprs_format::Limits::default()).unwrap().facts.features;
            let refused = wprs_format::features::refused("desktop", &facts);
            let thumbs = vec![(256, dir.path().join(format!("{name}.jpg")))];
            let response = handle(&Request { path: wpak.clone(), kind: Kind::Package, thumbs });
            let error = response.meta.error.unwrap_or_default();
            assert_eq!(response.written, [256], "{name}");
            let materialized = crate::apply::materialize(&wpak, &dir.path().join(format!("{name}.blob")));
            if refused.is_empty() {
                assert!(error.is_empty(), "{name}: {error}");
                let failed = materialized.err().map(|e| e.to_string()).unwrap_or_default();
                assert!(!failed.contains("needs a newer version"), "{name}: {failed}");
                continue;
            }
            assert!(
                error.starts_with("this package needs a newer version of wallpaper-rs"),
                "{name}: {error}"
            );
            for feature in &refused {
                let why = format!("{feature} ({})", wprs_format::features::explain(feature));
                assert!(error.contains(&why), "{name}: {error}");
            }
            let refused = materialized.unwrap_err().to_string();
            assert!(refused.contains("needs a newer version"), "{refused}");
        }
    }
}
