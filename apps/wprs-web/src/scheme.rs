use crate::ffi::{Ptr, get, text};
use crate::glib::string;
use crate::state::{engine, live};
use std::collections::HashSet;
use std::io::Write;
use std::os::fd::{IntoRawFd, OwnedFd};
use std::path::PathBuf;
use wprs_web::Start;

pub fn package(start: &Start) -> Result<(HashSet<String>, Option<String>), String> {
    let mut package =
        wprs_format::open(&start.package, &wprs_format::Limits::default()).map_err(|e| e.to_string())?;
    let kind = package.manifest.kind == wprs_format::Kind::Web && package.manifest.id == start.id;
    if !kind || package.manifest.entry != start.entry {
        return Err("the package is not this web page".into());
    }
    let entries: HashSet<String> = package.entries.iter().map(|e| e.path.clone()).collect();
    let preload = match &start.preload {
        Some(path) => Some(
            String::from_utf8(package.read_file(path).map_err(|e| e.to_string())?)
                .map_err(|_| "the preload is not text")?,
        ),
        None => None,
    };
    Ok((entries, preload))
}

enum Found {
    Packed(PathBuf, String),
    User(PathBuf),
}

fn find(uri: &str) -> Option<Found> {
    let engine = engine()?;
    let uri = uri.split(['?', '#']).next()?;
    if let Some(rest) = uri.strip_prefix("wpak://") {
        let (id, path) = rest.split_once('/')?;
        let path = wprs_format::page::decoded(path)?;
        let path = if path.is_empty() { engine.start.entry.clone() } else { path };
        let fine =
            id == engine.start.id && engine.entries.contains(&path) && !path.starts_with("manifest.json");
        return fine.then(|| Found::Packed(engine.start.package.clone(), path));
    }
    let file = live(|l| l.files.get(uri).cloned())?;
    Some(Found::User(file))
}

fn opened(found: Found) -> std::io::Result<(OwnedFd, u64, String)> {
    match found {
        Found::User(path) => {
            use std::os::unix::fs::OpenOptionsExt;
            let file = std::fs::OpenOptions::new().read(true).custom_flags(libc_nofollow()).open(&path)?;
            let meta = file.metadata()?;
            if !meta.is_file() {
                return Err(std::io::ErrorKind::InvalidInput.into());
            }
            Ok((file.into(), meta.len(), wprs_web::mime(&path.to_string_lossy()).into()))
        }
        Found::Packed(package, path) => {
            let (reader, writer) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)?;
            let mut archive =
                zip::ZipArchive::new(std::fs::File::open(&package)?).map_err(std::io::Error::other)?;
            let size = archive.by_name(&path).map_err(std::io::Error::other)?.size();
            let kind = wprs_web::mime(&path).to_owned();
            std::thread::spawn(move || {
                let mut out = std::fs::File::from(writer);
                if let Ok(mut file) = archive.by_name(&path) {
                    let _ = std::io::copy(&mut file, &mut out).and_then(|_| out.flush());
                }
            });
            Ok((reader, size, kind))
        }
    }
}

fn libc_nofollow() -> i32 {
    rustix::fs::OFlags::NOFOLLOW.bits() as i32
}

pub unsafe extern "C" fn serve(request: Ptr, _data: Ptr) {
    let a = get();
    let uri = unsafe { string((a.webkit_uri_scheme_request_get_uri)(request)) };
    match find(&uri).map(opened) {
        Some(Ok((fd, size, kind))) => unsafe {
            let stream = (a.g_unix_input_stream_new)(fd.into_raw_fd(), 1);
            let response = (a.webkit_uri_scheme_response_new)(stream, size as i64);
            let kind = text(&kind);
            (a.webkit_uri_scheme_response_set_content_type)(response, kind.as_ptr());
            let headers = (a.soup_message_headers_new)(1);
            (a.soup_message_headers_append)(headers, c"Access-Control-Allow-Origin".as_ptr(), c"*".as_ptr());
            (a.webkit_uri_scheme_response_set_http_headers)(response, headers);
            (a.webkit_uri_scheme_request_finish_with_response)(request, response);
            (a.g_object_unref)(response);
            (a.g_object_unref)(stream);
        },
        _ => unsafe {
            let domain = (a.g_quark_from_static_string)(c"wprs-web".as_ptr());
            let error = (a.g_error_new_literal)(domain, 404, c"not in this package".as_ptr());
            (a.webkit_uri_scheme_request_finish_error)(request, error);
            (a.g_error_free)(error);
        },
    }
}
