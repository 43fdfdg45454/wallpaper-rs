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
