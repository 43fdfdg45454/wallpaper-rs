use super::*;
use crate::error::Error;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, Request, ResponseTemplate};
use wprs_core::dbus::error::{EXISTS, NOT_FOUND, STALE};
use wprs_format::{Keypair, Limits, Settings, Signer, add, build, init, keygen, publish, sign_root};
use wprs_providers::HttpFetch;

fn signer(key: &Keypair) -> Signer {
    Signer::new(&key.secret, None).unwrap()
}

fn repository(dir: &Path) -> String {
    let (root, online) = (keygen(None).unwrap(), keygen(None).unwrap());
    let (root_keys, online_keys) = (vec![root.public.clone()], vec![online.public.clone()]);
    let settings = Settings { name: "Example".into(), root_keys, online_keys, ..Settings::default() };
    let site = dir.join("site");
    init(&site, &settings, now()).unwrap();
    sign_root(&site, &signer(&root)).unwrap();
    for version in ["1.0.0", "1.1.0"] {
        let src = dir.join(version);
        fs::create_dir_all(&src).unwrap();
        let manifest = json!({"schema": wprs_format::SCHEMA_V1, "id": "org.example.dawn", "version": version,
            "title": "Dawn", "license": "CC0-1.0", "kind": "image", "entry": "image.png", "preview": "preview.png"});
        fs::write(src.join("manifest.json"), manifest.to_string()).unwrap();
        let green = image::RgbaImage::from_pixel(8, 4, image::Rgba([0, 255, 0, 255]));
        green.save(src.join("image.png")).unwrap();
        green.save(src.join("preview.png")).unwrap();
        let wpak = dir.join(format!("{version}.wpak"));
        build(&src, &wpak, &Limits::default()).unwrap();
        add(&site, &wpak, None, now()).unwrap();
    }
    publish(&site, &signer(&online), 7, now()).unwrap();
    root.fingerprint
}

async fn serve(dir: PathBuf) -> MockServer {
    let server = MockServer::start().await;
    let files = move |request: &Request| match fs::read(dir.join(request.url.path().trim_start_matches('/')))
    {
        Ok(bytes) => ResponseTemplate::new(200).set_body_bytes(bytes),
        Err(_) => ResponseTemplate::new(404),
    };
    Mock::given(any()).respond_with(files).mount(&server).await;
    server
}

fn name(error: wprs_format::Error) -> String {
    Error::from(error).parts().0
}

#[test]
fn specs_and_errors_are_checked() {
    assert_eq!(parse_spec("org.example.dawn").unwrap(), (None, "org.example.dawn", None));
    let full = parse_spec("0123abcd/org.example.dawn@1.0.0").unwrap();
    assert_eq!(full, (Some("0123abcd"), "org.example.dawn", Some("1.0.0")));
    for bad in ["", "dawn", "0123/org.example.dawn", "org.example.dawn@", "../org.x", "org.example.dawn@1/x"]
    {
        assert!(parse_spec(bad).is_err(), "{bad}");
    }
    assert_eq!(name(wprs_format::Error::Stale("0123".into())), STALE);
    let interrupted = wprs_format::Error::Io(io::ErrorKind::Interrupted.into());
    assert!(matches!(Error::from(interrupted), Error::Cancelled));
    assert!(newer("1.10.0", "1.9.0") && !newer("1.0.0", "1.0.0"));
}

#[tokio::test(flavor = "multi_thread")]
async fn repositories_install_over_http_with_progress() {
    let tmp = tempfile::tempdir().unwrap();
    let fingerprint = repository(tmp.path());
    let server = serve(tmp.path().join("site")).await;
    let (url, data) = (format!("{}/?fingerprint={fingerprint}", server.uri()), tmp.path().join("data"));
    let dest = tmp.path().join("preview");
    tokio::task::spawn_blocking(move || {
        let fetch = HttpFetch::new(Arc::default()).unwrap();
        let info = wprs_format::add_repo(&data, &fetch, &url, now()).unwrap();
        assert_eq!((info.packages, info.hosts.len()), (1, 1));
        assert_eq!(name(wprs_format::add_repo(&data, &fetch, &url, now()).unwrap_err()), EXISTS);
        let dry = wprs_format::install(&data, &Dry, "org.example.dawn", now(), false, false);
        assert!(matches!(dry, Err(wprs_format::Error::Io(e)) if e.kind() == io::ErrorKind::WouldBlock));
        let missing = wprs_format::install(&data, &Dry, "org.example.dusk", now(), false, false);
        assert_eq!(name(missing.unwrap_err()), NOT_FOUND);
        let seen = AtomicU64::new(0);
        let progress = Progress(fetch, |done| seen.store(done, Ordering::Relaxed));
        let spec = format!("{}/org.example.dawn@1.0.0", info.repo_id);
        let record = wprs_format::install(&data, &progress, &spec, now(), false, false).unwrap();
        assert_eq!((seen.load(Ordering::Relaxed), record.version.as_str()), (record.size, "1.0.0"));
        wprs_format::install(&data, &progress, "org.example.dawn", now(), false, false).unwrap();
        let older = wprs_format::install(&data, &Dry, &spec, now(), false, false).unwrap_err();
        assert!(Error::from(older).to_string().contains("confirm the downgrade"));
        assert!(
            wprs_format::preview(&data, &progress.0, &info.repo_id, "org.example.dawn", "1.1.0", &dest)
                .unwrap()
        );
        assert!(image::ImageReader::open(&dest).unwrap().with_guessed_format().unwrap().decode().is_ok());
    })
    .await
    .unwrap();
}
