use crate::App;
use crate::config::{Clients, Config};
use crate::db::{DAY, now};
use crate::error::{Problem, Res};
use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::HeaderMap;
use axum::http::request::Parts;
use serde_json::json;
use std::collections::HashMap;
use std::convert::Infallible;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::net::{IpAddr, SocketAddr};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

pub struct Rule {
    pub name: &'static str,
    pub burst: f64,
    pub seconds: f64,
}

pub const API: Rule = Rule { name: "api", burst: 50.0, seconds: 2.5 };
pub const LOGIN: Rule = Rule { name: "login", burst: 5.0, seconds: 60.0 };
pub const FAILS: Rule = Rule { name: "fails", burst: 10.0, seconds: 900.0 };
pub const REPORT: Rule = Rule { name: "report", burst: 5.0, seconds: 3600.0 };
pub const SYNC: Rule = Rule { name: "sync", burst: 60.0, seconds: 60.0 };

type Buckets = HashMap<(&'static str, String), (f64, Instant, Instant)>;

pub struct Limiter {
    on: bool,
    buckets: Mutex<(Buckets, usize)>,
}

impl Limiter {
    pub fn new(on: bool) -> Limiter {
        Limiter { on, buckets: Mutex::new((HashMap::new(), 10_000)) }
    }

    pub fn take(&self, rule: &Rule, key: &str) -> Res<()> {
        self.spend(rule, key, 1.0)
    }

    pub fn allow(&self, rule: &Rule, key: &str) -> Res<()> {
        self.spend(rule, key, 0.0)
    }

    fn spend(&self, rule: &Rule, key: &str, cost: f64) -> Res<()> {
        if !self.on {
            return Ok(());
        }
        let mut guard = self.buckets.lock().unwrap_or_else(PoisonError::into_inner);
        let ((buckets, most), at) = (&mut *guard, Instant::now());
        if buckets.len() > *most {
            buckets.retain(|_, (_, _, full)| *full > at);
            *most = (buckets.len() * 2).max(10_000);
        }
        let rate = rule.burst / rule.seconds;
        let (tokens, last, full) =
            buckets.entry((rule.name, key.to_string())).or_insert((rule.burst, at, at));
        *tokens = (*tokens + at.duration_since(*last).as_secs_f64() * rate).min(rule.burst);
        *last = at;
        let refused = *tokens < 1.0;
        *tokens -= if refused { 0.0 } else { cost };
        *full = at + Duration::from_secs_f64((rule.burst - *tokens) / rate);
        if refused { Err(Problem::busy(((1.0 - *tokens) / rate).ceil() as u64)) } else { Ok(()) }
    }
}

pub fn within(&(net, prefix): &(IpAddr, u8), ip: IpAddr) -> bool {
    let wide = |ip: IpAddr| match ip.to_canonical() {
        IpAddr::V4(v4) => (u128::from(u32::from(v4)) << 96, true),
        IpAddr::V6(v6) => (u128::from(v6), false),
    };
    let ((n, n4), (i, i4)) = (wide(net), wide(ip));
    let mask = !u128::MAX.checked_shr(prefix.into()).unwrap_or(0);
    n4 == i4 && n & mask == i & mask
}

pub fn client_ip(cfg: &Config, headers: &HeaderMap, peer: Option<SocketAddr>) -> IpAddr {
    let peer = peer.map_or(IpAddr::from([0, 0, 0, 0]), |p| p.ip());
    let trusted = |ip: IpAddr| cfg.trusted_proxies.iter().any(|net| within(net, ip));
    if !trusted(peer) {
        return peer;
    }
    let hops =
        headers.get_all("x-forwarded-for").iter().filter_map(|v| v.to_str().ok()).flat_map(|v| v.split(','));
    let hops: Vec<IpAddr> = hops.filter_map(|hop| hop.trim().parse().ok()).collect();
    hops.into_iter().rev().find(|ip| !trusted(*ip)).unwrap_or(peer)
}

pub fn key(ip: IpAddr, v4: u8, v6: u8) -> String {
    match ip.to_canonical() {
        IpAddr::V4(a) => {
            IpAddr::from((u32::from(a) & !u32::MAX.checked_shr(v4.into()).unwrap_or(0)).to_be_bytes())
        }
        IpAddr::V6(a) => {
            IpAddr::from((u128::from(a) & !u128::MAX.checked_shr(v6.into()).unwrap_or(0)).to_be_bytes())
        }
    }
    .to_string()
}

pub struct Client(pub IpAddr);

impl Client {
    pub fn key(&self) -> String {
        key(self.0, 32, 64)
    }
}

impl FromRequestParts<Arc<App>> for Client {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, app: &Arc<App>) -> Result<Client, Infallible> {
        let peer = parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|c| c.0);
        Ok(Client(client_ip(&app.cfg, &parts.headers, peer)))
    }
}

pub struct ClientLog {
    mode: Clients,
    days: u64,
    dir: PathBuf,
    file: Mutex<Option<(String, File)>>,
}

impl ClientLog {
    pub fn new(cfg: &Config) -> ClientLog {
        ClientLog {
            mode: cfg.log_clients,
            days: cfg.log_days,
            dir: cfg.data.join("logs"),
            file: Mutex::default(),
        }
    }

    pub fn write(&self, ip: IpAddr, agent: Option<&str>, method: &str, route: &str, status: u16) {
        let ip = match self.mode {
            Clients::Off => return,
            Clients::Prefix => key(ip, 24, 48),
            Clients::Full => ip.to_string(),
        };
        let agent = agent.filter(|_| self.mode == Clients::Full);
        let day =
            chrono::DateTime::from_timestamp_millis(now()).unwrap_or_default().format("%Y-%m-%d").to_string();
        let line = json!({
            "at": now(), "ip": ip, "agent": agent, "method": method, "route": route, "status": status,
        });
        let mut slot = self.file.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.as_ref().is_none_or(|(d, _)| *d != day) {
            let path = self.dir.join(format!("clients-{day}.jsonl"));
            *slot =
                OpenOptions::new().create(true).append(true).mode(0o600).open(path).ok().map(|f| (day, f));
        }
        if let Some((_, file)) = slot.as_mut() {
            let _ = writeln!(file, "{line}");
        }
    }

    pub fn purge(&self) {
        let cutoff =
            chrono::DateTime::from_timestamp_millis(now() - self.days as i64 * DAY).unwrap_or_default();
        let cutoff = format!("clients-{}.jsonl", cutoff.format("%Y-%m-%d"));
        for item in fs::read_dir(&self.dir).into_iter().flatten().flatten() {
            let name = item.file_name().to_string_lossy().into_owned();
            if name.starts_with("clients-") && name < cutoff {
                let _ = fs::remove_file(item.path());
            }
        }
    }
}
