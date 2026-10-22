use crate::config::{Clients, Config, Registration};
use crate::creds::{token, user_code, well_formed};
use crate::net::{API, LOGIN, Limiter, client_ip, key, within};
use axum::http::HeaderMap;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};

fn config(vars: &[(&str, &str)]) -> anyhow::Result<Config> {
    let vars: HashMap<String, String> = vars.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    Config::from_vars(|key| vars.get(key).cloned())
}

#[test]
fn tokens_carry_a_checksum() {
    let token = token('d');
    assert!(token.starts_with("wprs_d_") && token.len() == 80 && well_formed(&token));
    let mut forged = token.clone();
    forged.replace_range(10..11, if &token[10..11] == "0" { "1" } else { "0" });
    assert!(!well_formed(&forged) && !well_formed("wprs_d_short_00000000"));
    let code = user_code();
    assert!(
        code.len() == 9
            && code.as_bytes()[4] == b'-'
            && code.chars().all(|c| c == '-' || "BCDFGHJKLMNPQRSTVWXZ".contains(c))
    );
}

#[test]
fn configuration_defaults_and_errors() {
    let cfg = config(&[]).unwrap();
    assert_eq!(
        (cfg.registration, cfg.log_clients, cfg.entry_days, cfg.cookie()),
        (Registration::Invite, Clients::Off, 7, "wprs")
    );
    assert_eq!(cfg.licenses, ["CC0-1.0", "CC-BY-4.0", "CC-BY-SA-4.0"]);
    let secure = config(&[
        ("WPRS_PUBLIC_URL", "https://example.org/"),
        ("WPRS_TRUSTED_PROXIES", "172.16.0.0/12, ::1"),
    ])
    .unwrap();
    assert_eq!(
        (secure.cookie(), secure.url("/x").as_str(), secure.trusted_proxies.len()),
        ("__Host-wprs", "https://example.org/x", 2)
    );
    assert!(
        config(&[("WPRS_REGISTRATION", "everyone")])
            .unwrap_err()
            .to_string()
            .contains("closed, invite, open")
    );
    assert!(config(&[("WPRS_TRUSTED_PROXIES", "10.0.0.0/40")]).is_err());
    assert!(config(&[("WPRS_ENTRY_DAYS", "30")]).is_err());
    assert!(config(&[("WPRS_PUBLIC_URL", "example.org")]).is_err());
}

#[test]
fn proxies_and_addresses() {
    let ip = |text: &str| text.parse::<IpAddr>().unwrap();
    assert!(
        within(&(ip("172.16.0.0"), 12), ip("172.20.1.2"))
            && !within(&(ip("172.16.0.0"), 12), ip("172.32.0.1"))
    );
    assert!(within(&(ip("10.0.0.0"), 8), ip("::ffff:10.1.2.3")) && within(&(ip("::"), 0), ip("2001:db8::1")));
    assert_eq!(
        (key(ip("192.0.2.77"), 24, 48), key(ip("2001:db8:1:2:3::9"), 32, 64)),
        ("192.0.2.0".to_string(), "2001:db8:1:2::".to_string())
    );
    let cfg = config(&[("WPRS_TRUSTED_PROXIES", "10.0.0.0/8")]).unwrap();
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", "198.51.100.9, 203.0.113.5, 10.0.0.3".parse().unwrap());
    let peer = |text: &str| Some(SocketAddr::new(ip(text), 4000));
    assert_eq!(client_ip(&cfg, &headers, peer("10.0.0.2")), ip("203.0.113.5"));
    assert_eq!(client_ip(&cfg, &headers, peer("192.0.2.1")), ip("192.0.2.1"));
}

#[test]
fn limiter_spends_and_refuses() {
    let limits = Limiter::new(true);
    (0..5).for_each(|_| limits.take(&LOGIN, "a").unwrap());
    let refused = limits.take(&LOGIN, "a").unwrap_err();
    assert_eq!((refused.status.as_u16(), refused.retry.is_some_and(|s| s <= 12)), (429, true));
    assert!(limits.take(&LOGIN, "b").is_ok() && limits.allow(&API, "a").is_ok());
    let off = Limiter::new(false);
    assert!((0..100).all(|_| off.take(&LOGIN, "a").is_ok()));
}

fn app(dir: &std::path::Path, vars: &[(&str, &str)]) -> std::sync::Arc<crate::App> {
    let data = dir.display().to_string();
    let cfg = config(&[&[("WPRS_DATA", data.as_str())], vars].concat()).unwrap();
    crate::App::open(cfg).unwrap()
}

#[tokio::test]
async fn notices_forget_their_sender_after_the_retention() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(dir.path(), &[("WPRS_NOTICE_DAYS", "30")]);
    let sql = "INSERT INTO notices (id, kind, name, email, location, work, explanation, signature, received_at) \
               VALUES (?1, 'copyright', 'Ana Example', 'ana@example.org', 'x', 'y', 'z', 'Ana Example', ?2)";
    let t = crate::db::now();
    let rows = [("old", t - 31 * crate::db::DAY), ("new", t - crate::db::DAY)];
    app.db
        .run(move |c| {
            Ok(rows.iter().try_for_each(|(id, at)| c.execute(sql, rusqlite::params![id, at]).map(drop))?)
        })
        .await
        .unwrap();
    crate::jobs::purge(&app).await.unwrap();
    let kept = app
        .db
        .run(|c| crate::db::rows(c, "SELECT id, name, email, signature, work FROM notices ORDER BY id", []))
        .await
        .unwrap();
    assert_eq!(
        kept[0],
        serde_json::json!({"id": "new", "name": "Ana Example", "email": "ana@example.org", "signature": "Ana Example", "work": "y"})
    );
    assert_eq!(
        kept[1],
        serde_json::json!({"id": "old", "name": "", "email": "", "signature": "", "work": "y"})
    );
}

#[test]
fn backups_hold_exactly_the_files_their_database_names() {
    let dir = tempfile::tempdir().unwrap();
    let app = app(&dir.path().join("data"), &[]);
    let (cfg, sha) = (app.cfg.clone(), "ab".repeat(32));
    let conn = rusqlite::Connection::open(cfg.data.join("db/wprs.sqlite")).unwrap();
    let sql = "INSERT INTO users (handle, role, quota_bytes, created_at) VALUES ('ana', 'user', 1, 0);
               INSERT INTO packages (id, owner_id, created_at) VALUES ('org.example.dawn', 1, 0);";
    conn.execute_batch(sql).unwrap();
    let sql = "INSERT INTO versions (package_id, version, sha256, size, kind, title, description, license, tags, \
               authors, state, uploaded_by, created_at) VALUES ('org.example.dawn', '1.0.0', ?1, 1, 'image', 't', \
               'd', 'CC0-1.0', '[]', '[]', 'pending', 1, 0)";
    conn.execute(sql, [&sha]).unwrap();
    for name in
        [format!("blobs/{sha}.wpak"), format!("thumbs/{sha}.jpg"), format!("blobs/{}.wpak", "cd".repeat(32))]
    {
        std::fs::write(cfg.data.join(name), b"x").unwrap();
    }
    let saved = crate::backup(&cfg, Some(dir.path().join("one"))).unwrap();
    assert!(
        saved.join(format!("blobs/{sha}.wpak")).exists() && saved.join(format!("thumbs/{sha}.jpg")).exists()
    );
    assert!(!saved.join(format!("blobs/{}.wpak", "cd".repeat(32))).exists());
    std::fs::remove_file(cfg.data.join(format!("blobs/{sha}.wpak"))).unwrap();
    std::fs::write(cfg.data.join(format!("held/{sha}.wpak")), b"x").unwrap();
    let saved = crate::backup(&cfg, Some(dir.path().join("two"))).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(saved.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["missing"], serde_json::json!([format!("blobs/{sha}.wpak")]));
    assert!(manifest["files"].get(format!("held/{sha}.wpak")).is_some());
    let restored = Config { data: dir.path().join("restored"), ..cfg.clone() };
    crate::restore(&restored, &saved, false).unwrap();
    assert!(restored.data.join(format!("held/{sha}.wpak")).is_file());
    assert!(crate::backup(&cfg, Some(saved)).unwrap_err().to_string().contains("not empty"));
}
