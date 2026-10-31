use crate::config::Config;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use openidconnect::core::{
    CoreEdDsaPrivateSigningKey, CoreIdToken, CoreIdTokenClaims, CoreJwsSigningAlgorithm,
};
use openidconnect::{
    Audience, EmptyAdditionalClaims, EndUserUsername, IssuerUrl, JsonWebKeyId, Nonce, PrivateSigningKey,
    StandardClaims, SubjectIdentifier,
};
use reqwest::StatusCode;
use serde_json::json;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use wiremock::matchers::path;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn key(seed: u8) -> CoreEdDsaPrivateSigningKey {
    let prefix =
        [0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20];
    let pem = BASE64_STANDARD.encode([&prefix[..], &[seed; 32]].concat());
    let pem = format!("-----BEGIN PRIVATE KEY-----\n{pem}\n-----END PRIVATE KEY-----\n");
    CoreEdDsaPrivateSigningKey::from_ed25519_pem(&pem, Some(JsonWebKeyId::new(format!("k{seed}")))).unwrap()
}

async fn issuer(mock: &MockServer, signer: &CoreEdDsaPrivateSigningKey, nonce: Option<&str>) {
    let base = mock.uri();
    mock.reset().await;
    let metadata = json!({
        "issuer": base, "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"), "jwks_uri": format!("{base}/jwks"),
        "response_types_supported": ["code"], "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["EdDSA"],
    });
    let keys = json!({"keys": [signer.as_verification_key()]});
    let routes = [("/.well-known/openid-configuration", metadata), ("/jwks", keys)];
    for (at, body) in routes {
        Mock::given(path(at)).respond_with(ResponseTemplate::new(200).set_body_json(body)).mount(mock).await;
    }
    let Some(nonce) = nonce else { return };
    let now = chrono::Utc::now();
    let who = StandardClaims::new(SubjectIdentifier::new("ana-subject".into()))
        .set_preferred_username(Some(EndUserUsername::new("ana".into())));
    let claims = CoreIdTokenClaims::new(
        IssuerUrl::new(base).unwrap(),
        vec![Audience::new("wprs".into())],
        now + chrono::Duration::minutes(5),
        now,
        who,
        EmptyAdditionalClaims {},
    )
    .set_nonce(Some(Nonce::new(nonce.into())));
    let token = CoreIdToken::new(claims, signer, CoreJwsSigningAlgorithm::EdDsa, None, None).unwrap();
    let answer = json!({"access_token": "a", "token_type": "Bearer", "id_token": token.to_string()});
    Mock::given(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(answer))
        .mount(mock)
        .await;
}

async fn sign_in(
    url: &str,
    http: &reqwest::Client,
    mock: &MockServer,
    signer: &CoreEdDsaPrivateSigningKey,
) -> StatusCode {
    let started = http.get(format!("{url}/api/v1/auth/oidc/start")).send().await.unwrap();
    let cookie = started.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_owned();
    let location = reqwest::Url::parse(started.headers()["location"].to_str().unwrap()).unwrap();
    let query: HashMap<_, _> = location.query_pairs().into_owned().collect();
    issuer(mock, signer, Some(&query["nonce"])).await;
    let back = format!("{url}/api/v1/auth/oidc/callback?code=c&state={}", query["state"]);
    http.get(back).header("cookie", cookie).send().await.unwrap().status()
}

#[tokio::test]
async fn sign_in_survives_a_key_rotation_of_the_issuer() {
    let (mock, dir) = (MockServer::start().await, tempfile::tempdir().unwrap());
    let (old, new) = (key(1), key(2));
    issuer(&mock, &old, None).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (data, base) = (dir.path().display().to_string(), mock.uri());
    let vars: HashMap<&str, &str> = HashMap::from([
        ("WPRS_DATA", data.as_str()),
        ("WPRS_PUBLIC_URL", url.as_str()),
        ("WPRS_RATE_LIMITS", "false"),
        ("WPRS_OIDC_ISSUER", base.as_str()),
        ("WPRS_OIDC_CLIENT_ID", "wprs"),
        ("WPRS_OIDC_CLIENT_SECRET", "secret"),
        ("WPRS_OIDC_AUTO_PROVISION", "true"),
    ]);
    let app = crate::App::open(Config::from_vars(|k| vars.get(k).map(|v| v.to_string())).unwrap()).unwrap();
    tokio::spawn(app.clone().serve(listener));
    let http = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap();
    assert_eq!(sign_in(&url, &http, &mock, &old).await, StatusCode::SEE_OTHER);
    let fetched = || async {
        mock.received_requests().await.unwrap().iter().filter(|r| r.url.path() == "/jwks").count()
    };
    assert_eq!(sign_in(&url, &http, &mock, &new).await, StatusCode::FORBIDDEN);
    assert_eq!(fetched().await, 0, "the key set is fetched again at most once a minute");
    let aged = Instant::now().checked_sub(Duration::from_secs(61)).unwrap();
    app.oidc.found.lock().unwrap().as_mut().unwrap().1 = aged;
    assert_eq!(sign_in(&url, &http, &mock, &new).await, StatusCode::SEE_OTHER);
    assert_eq!(fetched().await, 1);
    assert_eq!(sign_in(&url, &http, &mock, &old).await, StatusCode::FORBIDDEN);
    let sql = "SELECT COUNT(*) FROM identities WHERE subject = 'ana-subject'";
    let bound: i64 = app.db.run(move |c| Ok(c.query_row(sql, [], |r| r.get(0))?)).await.unwrap();
    assert_eq!(bound, 1);
}
