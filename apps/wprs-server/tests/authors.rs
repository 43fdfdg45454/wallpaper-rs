mod common;

use common::{HttpFetch, package, start, upload};
use reqwest::{Method, StatusCode};
use serde_json::json;
use std::fs;
use std::path::Path;
use wprs_format::{
    Error, Keypair, Signer, add, add_repo, install, keygen, sign_rotation, sign_source, update_repo,
};

const DAWN: &str = "org.example.dawn";

fn signed(dir: &Path, version: &str, key: &Keypair) -> std::path::PathBuf {
    let wpak = package(dir, DAWN, version);
    let src = dir.join(format!("src-{DAWN}-{version}"));
    sign_source(&src, &Signer::new(&key.secret, None).unwrap()).unwrap();
    wprs_format::build(&src, &wpak, &wprs_format::Limits::default()).unwrap();
    wpak
}

fn later(seconds: i64) -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64 + seconds
}

#[tokio::test(flavor = "multi_thread")]
async fn online_authors_register_rotate_and_survive_a_stolen_online_key() {
    let server = start(&[("WPRS_AUTO_PUBLISH", "trusted")]).await;
    wprs_server::init(&server.cfg, "Example wallpapers", true, true).unwrap();
    let admin = server.admin().await;
    let ana = server.user(&admin, "ana", "trusted").await;
    let (_, me) = server.json(Method::GET, "/api/v1/me", &ana, None).await;
    let author = format!("u{}", me["id"]);
    let (first, next) = (keygen(None).unwrap(), keygen(None).unwrap());
    let body = json!({"public_key": first.public});
    let (status, _) = server.json(Method::POST, "/api/v1/me/author-keys", &ana, Some(body)).await;
    assert_eq!(status, StatusCode::CREATED);
    let published = upload(&server, &ana, &signed(server.dir.path(), "1.0.0", &first)).await;
    assert_eq!(published["state"], "published", "{published}");
    server.json(Method::POST, "/api/v1/admin/repo/publish", &admin, None).await;
    let url = wprs_server::status(&server.cfg).unwrap()["url"].as_str().unwrap().to_string();
    let (data, repo) = (server.dir.path().join("client"), server.dir.path().join("data/repo"));
    let repo_id = {
        let (data, url, fp) = (data.clone(), url.clone(), first.fingerprint.clone());
        tokio::task::spawn_blocking(move || {
            let info = add_repo(&data, &HttpFetch, &url, later(0)).unwrap();
            let done = install(&data, &HttpFetch, DAWN, later(0), false, false).unwrap();
            assert_eq!(done.signer, Some(fp));
            info.repo_id
        })
        .await
        .unwrap()
    };
    let entry = fs::read_to_string(repo.join("entry/latest")).unwrap();
    let pointed = fs::read_to_string(repo.join(format!("entry/{}.json", entry.trim()))).unwrap();
    assert!(pointed.contains("\"authors\":{"), "{pointed}");

    let thief = keygen(None).unwrap();
    wprs_format::register_author(&repo, &author, &thief.public).unwrap();
    add(&repo, &signed(server.dir.path(), "1.1.0", &thief), None, later(0)).unwrap();
    let online =
        Signer::new(&fs::read_to_string(server.dir.path().join("data/keys/online.key")).unwrap(), None);
    wprs_format::publish(&repo, &online.unwrap(), 7, later(0)).unwrap();
    let refused = {
        let (data, repo_id) = (data.clone(), repo_id.clone());
        tokio::task::spawn_blocking(move || {
            update_repo(&data, &HttpFetch, &repo_id, later(2)).unwrap();
            install(&data, &HttpFetch, DAWN, later(2), false, false)
        })
        .await
        .unwrap()
    };
    assert!(matches!(&refused, Err(Error::Signature(m)) if m.contains("hand it over")), "{refused:?}");

    let sig = sign_rotation(&Signer::new(&first.secret, None).unwrap(), &author, &next.public).unwrap();
    let forged = sign_rotation(&Signer::new(&thief.secret, None).unwrap(), &author, &next.public).unwrap();
    let rotate = |signature: &str| json!({"from": first.fingerprint, "public_key": next.public, "signature": signature});
    let (status, _) =
        server.json(Method::POST, "/api/v1/me/author-keys/rotate", &ana, Some(rotate(&forged))).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, rotated) =
        server.json(Method::POST, "/api/v1/me/author-keys/rotate", &ana, Some(rotate(&sig))).await;
    assert_eq!(
        (status, rotated["fingerprint"].as_str()),
        (StatusCode::CREATED, Some(next.fingerprint.as_str()))
    );
    let published = upload(&server, &ana, &signed(server.dir.path(), "1.2.0", &next)).await;
    assert_eq!(published["state"], "published", "{published}");
    let (status, _) = server.json(Method::POST, "/api/v1/admin/repo/publish", &admin, None).await;
    assert_eq!(status, StatusCode::OK);
    let done = tokio::task::spawn_blocking(move || {
        update_repo(&data, &HttpFetch, &repo_id, later(3)).unwrap();
        install(&data, &HttpFetch, DAWN, later(3), false, false).unwrap()
    });
    let done = done.await.unwrap();
    assert_eq!((done.version.as_str(), done.signer.as_deref()), ("1.2.0", Some(next.fingerprint.as_str())));
    let (_, keys) = server.json(Method::GET, "/api/v1/me/author-keys", &ana, None).await;
    assert_eq!(keys["items"].as_array().unwrap().len(), 2, "{keys}");
    let gone = format!("/api/v1/me/author-keys/{}", first.fingerprint);
    assert_eq!(
        server.call(Method::DELETE, &gone, &ana).send().await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
}
