use super::{Clients, Config, Oidc, Publish, Registration};
use anyhow::{Context, Result, ensure};
use std::collections::HashMap;
use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;

impl Config {
    pub fn from_env() -> Result<Config> {
        let mut vars = HashMap::new();
        for (key, value) in std::env::vars().filter(|(k, _)| k.starts_with("WPRS_")) {
            match key.strip_suffix("_FILE") {
                Some(base) => {
                    let text = fs::read_to_string(&value).with_context(|| format!("cannot read {key}"))?;
                    vars.insert(base.to_string(), text.trim().to_string());
                }
                None => drop(vars.entry(key).or_insert(value)),
            }
        }
        Config::from_vars(|key| vars.get(key).filter(|v| !v.trim().is_empty()).cloned())
    }

    pub fn from_vars(lookup: impl Fn(&str) -> Option<String>) -> Result<Config> {
        let get = |key: &str| lookup(&format!("WPRS_{key}"));
        let text = |key: &str, default: &str| get(key).unwrap_or_else(|| default.into());
        let num = |key: &str, default: u64| -> Result<u64> {
            get(key).map_or(Ok(default), |v| {
                v.trim().parse().with_context(|| format!("WPRS_{key}: not a number"))
            })
        };
        let flag = |key: &str, default: bool| -> Result<bool> {
            let value = get(key).map(|v| v.trim().to_ascii_lowercase());
            pick(
                key,
                value.as_deref(),
                &[("true", true), ("1", true), ("on", true), ("false", false), ("0", false), ("off", false)],
                default,
            )
        };
        let public_url = text("PUBLIC_URL", "http://localhost:8080").trim_end_matches('/').to_string();
        let scheme = public_url.starts_with("https://") || public_url.starts_with("http://");
        ensure!(scheme, "WPRS_PUBLIC_URL must start with https:// or http://");
        let proxies = text("TRUSTED_PROXIES", "");
        let oidc = get("OIDC_ISSUER").map(|issuer| -> Result<Oidc> {
            ensure!(cfg!(feature = "oidc"), "WPRS_OIDC_ISSUER is set but this build has no OIDC support");
            Ok(Oidc {
                issuer,
                client_id: get("OIDC_CLIENT_ID").context("WPRS_OIDC_CLIENT_ID is required with OIDC")?,
                client_secret: get("OIDC_CLIENT_SECRET")
                    .context("WPRS_OIDC_CLIENT_SECRET is required with OIDC")?,
                auto_provision: flag("OIDC_AUTO_PROVISION", false)?,
            })
        });
        let entry_days = num("ENTRY_DAYS", 7)?;
        ensure!((1..=14).contains(&entry_days), "WPRS_ENTRY_DAYS must be 1 to 14");
        Ok(Config {
            listen: text("LISTEN", "0.0.0.0:8080").parse().context("WPRS_LISTEN: expected address:port")?,
            data: PathBuf::from(text("DATA", "/data")),
            trusted_proxies: proxies
                .split(',')
                .filter(|p| !p.trim().is_empty())
                .map(cidr)
                .collect::<Result<_>>()?,
            name: text("NAME", "wallpaper-rs"),
            registration: pick(
                "REGISTRATION",
                get("REGISTRATION").as_deref(),
                &[
                    ("closed", Registration::Closed),
                    ("invite", Registration::Invite),
                    ("open", Registration::Open),
                ],
                Registration::Invite,
            )?,
            auto_publish: pick(
                "AUTO_PUBLISH",
                get("AUTO_PUBLISH").as_deref(),
                &[("never", Publish::Never), ("trusted", Publish::Trusted), ("all", Publish::All)],
                Publish::Trusted,
            )?,
            licenses: text("LICENSES", "CC0-1.0,CC-BY-4.0,CC-BY-SA-4.0")
                .split(',')
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect(),
            package_max: num("PACKAGE_MAX_MIB", 256)? << 20,
            user_quota: num("USER_QUOTA_MIB", 2048)? << 20,
            uploads_per_day: num("UPLOADS_PER_DAY", 20)?,
            sync_quota: num("SYNC_QUOTA_MIB", 64)? << 20,
            files_quota: num("FILES_QUOTA_MIB", 1024)? << 20,
            entry_days: entry_days as u32,
            resign_hours: num("RESIGN_HOURS", 24)?.clamp(1, entry_days * 12),
            publish_delay: num("PUBLISH_DELAY_SECS", 30)?,
            operator: text("LEGAL_OPERATOR", ""),
            contact: text("LEGAL_CONTACT", ""),
            jurisdiction: text("LEGAL_JURISDICTION", ""),
            dmca_agent: text("LEGAL_DMCA_AGENT", ""),
            oidc: oidc.transpose()?,
            log_clients: pick(
                "LOG_CLIENTS",
                get("LOG_CLIENTS").as_deref(),
                &[("off", Clients::Off), ("prefix", Clients::Prefix), ("full", Clients::Full)],
                Clients::Off,
            )?,
            log_days: num("LOG_CLIENTS_DAYS", 7)?.clamp(1, 30),
            notice_days: num("NOTICE_DAYS", 180)?.clamp(1, 730),
            takedown_days: num("TAKEDOWN_DAYS", 90)?.clamp(1, 3650),
            rate_limits: flag("RATE_LIMITS", true)?,
            validator: std::env::current_exe().unwrap_or_else(|_| PathBuf::from("wprs-server")),
            public_url,
        })
    }

    pub fn secure(&self) -> bool {
        self.public_url.starts_with("https://")
    }

    pub fn cookie(&self) -> &'static str {
        if self.secure() { "__Host-wprs" } else { "wprs" }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.public_url)
    }
}

fn pick<T: Copy>(key: &str, value: Option<&str>, options: &[(&str, T)], default: T) -> Result<T> {
    let Some(value) = value.map(str::trim) else { return Ok(default) };
    let names: Vec<&str> = options.iter().map(|(name, _)| *name).collect();
    let found = options.iter().find(|(name, _)| *name == value).map(|(_, v)| *v);
    found.with_context(|| format!("WPRS_{key} must be one of {}", names.join(", ")))
}

fn cidr(text: &str) -> Result<(IpAddr, u8)> {
    let (ip, bits) = text.trim().split_once('/').map_or((text.trim(), None), |(a, b)| (a, Some(b)));
    let ip: IpAddr = ip.parse().with_context(|| format!("WPRS_TRUSTED_PROXIES: invalid address {text:?}"))?;
    let max = if ip.is_ipv4() { 32 } else { 128 };
    let bits = bits
        .map_or(Ok(max), str::parse)
        .with_context(|| format!("WPRS_TRUSTED_PROXIES: invalid prefix {text:?}"))?;
    ensure!(bits <= max, "WPRS_TRUSTED_PROXIES: prefix too long in {text:?}");
    Ok((ip, bits))
}
