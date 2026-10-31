use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

mod load;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Registration {
    Closed,
    Invite,
    Open,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Publish {
    Never,
    Trusted,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clients {
    Off,
    Prefix,
    Full,
}

#[derive(Clone, Debug)]
pub struct Oidc {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    pub auto_provision: bool,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub public_url: String,
    pub listen: SocketAddr,
    pub data: PathBuf,
    pub trusted_proxies: Vec<(IpAddr, u8)>,
    pub name: String,
    pub registration: Registration,
    pub auto_publish: Publish,
    pub licenses: Vec<String>,
    pub package_max: u64,
    pub user_quota: u64,
    pub uploads_per_day: u64,
    pub sync_quota: u64,
    pub files_quota: u64,
    pub entry_days: u32,
    pub resign_hours: u64,
    pub publish_delay: u64,
    pub operator: String,
    pub contact: String,
    pub jurisdiction: String,
    pub dmca_agent: String,
    pub oidc: Option<Oidc>,
    pub log_clients: Clients,
    pub log_days: u64,
    pub notice_days: u64,
    pub takedown_days: u64,
    pub rate_limits: bool,
    pub validator: PathBuf,
}
