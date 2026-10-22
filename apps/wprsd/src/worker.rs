use crate::decode::{Picture, Problem, color, decode, package, video};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageReader};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::os::fd::FromRawFd;
use std::path::{Path, PathBuf};
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
    let picture = match request.kind {
        Kind::Image => {
            ImageReader::open(&request.path).map_err(Problem::from).and_then(decode).map(Picture::of)
        }
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
                meta.color = Some(color(&image));
                if let Err(e) = thumbnails(image, &request.thumbs, &mut written) {
                    return Response { meta, written, failure: Some(e.to_string()), missing: false };
                }
            }
            Response { meta, written, failure, missing: false }
        }
    }
}

fn thumbnails(
    mut image: DynamicImage,
    thumbs: &[(u32, PathBuf)],
    written: &mut Vec<u32>,
) -> anyhow::Result<()> {
    let mut thumbs = thumbs.to_vec();
    thumbs.sort_by_key(|(size, _)| std::cmp::Reverse(*size));
    for (size, dest) in thumbs {
        if image.width().max(image.height()) > size {
            image = image.resize(size, size, image::imageops::FilterType::Triangle);
        }
        let dir = dest.parent().unwrap_or(Path::new("."));
        let mut file = tempfile::Builder::new().prefix(".tmp-").tempfile_in(dir)?;
        image.to_rgb8().write_with_encoder(JpegEncoder::new_with_quality(&mut file, 85))?;
        file.persist(&dest)?;
        written.push(size);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::SCENE;
    use image::RgbaImage;

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
    }

    fn package(dir: &Path, kind: &str, entry: &str, preview: Option<&str>) -> PathBuf {
        let source = dir.join(kind);
        std::fs::create_dir(&source).unwrap();
        RgbaImage::from_pixel(80, 40, image::Rgba([255, 0, 0, 255])).save(source.join("image.png")).unwrap();
        RgbaImage::from_pixel(20, 20, image::Rgba([0, 0, 255, 255]))
            .save(source.join("preview.png"))
            .unwrap();
        if kind == "scene" {
            std::fs::write(source.join("scene.json"), "{}").unwrap();
        }
        let manifest = serde_json::json!({"schema": wprs_format::SCHEMA, "id": "org.example.calm", "version": "1.0.0",
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
        assert_eq!(
            (response.meta.error.as_deref(), response.failure.as_deref()),
            (Some(SCENE), Some("no preview"))
        );
        assert!(crate::apply::materialize(&scene, &blob.with_file_name("scene")).is_err());
    }
}
