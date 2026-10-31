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
    wprs_scene::launch_checks_from_here();
    let replies = unsafe {
        libc::setpriority(libc::PRIO_PROCESS, 0, 19);
        libc::syscall(libc::SYS_ioprio_set, 1, 0, 3 << 13);
        let replies = libc::fcntl(1, libc::F_DUPFD_CLOEXEC, 3);
        anyhow::ensure!(replies >= 0, "cannot set up the reply channel");
        libc::dup2(2, 1);
        std::fs::File::from_raw_fd(replies)
    };
    crate::compositor::stops::restore();
    let mut out = std::io::BufWriter::new(replies);
    for line in std::io::stdin().lock().lines() {
        let request: Request = serde_json::from_str(&line?)?;
        serde_json::to_writer(&mut out, &handle(&request))?;
        out.write_all(b"\n")?;
        out.flush()?;
    }
    wprs_scene::end_trials();
    wprs_render::finish_builds(0);
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
mod tests;
