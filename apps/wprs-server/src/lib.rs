mod account;
mod admin;
mod auth;
mod backup;
mod catalog;
mod config;
mod creds;
mod db;
mod device;
mod error;
mod http;
mod jobs;
mod legal;
mod mailbox;
mod me;
mod moderation;
mod net;
#[cfg(feature = "oidc")]
mod oidc;
mod pages;
mod records;
mod repo;
mod review;
mod sync;
#[cfg(test)]
mod tests;
mod upload;
mod validate;
mod web;

pub use backup::{backup, restore};
pub use config::Config;
pub use repo::{import, init, status};
pub use validate::check;

use rusqlite::params;
use std::fs::DirBuilder;
use std::net::SocketAddr;
use std::os::unix::fs::DirBuilderExt;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, Notify, Semaphore};

type St = axum::extract::State<Arc<App>>;
type Router = axum::Router<Arc<App>>;
type Reply = error::Res<axum::Json<serde_json::Value>>;

pub struct App {
    pub cfg: Config,
    db: db::Db,
    repo: repo::Repo,
    limits: net::Limiter,
    hub: mailbox::Hub,
    clients: net::ClientLog,
    validations: Semaphore,
    hashing: Semaphore,
    dirty: Notify,
    deciding: Mutex<()>,
    #[cfg(feature = "oidc")]
    oidc: oidc::Oidc,
}

impl App {
    pub fn open(cfg: Config) -> anyhow::Result<Arc<App>> {
        for dir in ["db", "blobs", "thumbs", "tmp", "keys", "repo", "logs", "backups"] {
            DirBuilder::new().recursive(true).mode(0o700).create(cfg.data.join(dir))?;
        }
        Ok(Arc::new(App {
            db: db::Db::open(&cfg.data.join("db/wprs.sqlite"))?,
            repo: repo::Repo::new(&cfg.data),
            limits: net::Limiter::new(cfg.rate_limits),
            hub: mailbox::Hub::default(),
            clients: net::ClientLog::new(&cfg),
            validations: Semaphore::new(2),
            hashing: Semaphore::new(4),
            dirty: Notify::new(),
            deciding: Mutex::new(()),
            #[cfg(feature = "oidc")]
            oidc: oidc::Oidc::default(),
            cfg,
        }))
    }

    pub fn router(self: &Arc<App>) -> axum::Router {
        http::router(self.clone())
    }

    pub async fn serve(self: Arc<App>, listener: TcpListener) -> anyhow::Result<()> {
        let admins = self.db.run(|c| {
            Ok(c.query_row("SELECT COUNT(*) FROM users WHERE role = 'admin'", [], |r| r.get::<_, i64>(0))?)
        });
        let admins = admins.await.map_err(|e| anyhow::anyhow!(e.detail))?;
        if admins == 0 {
            let token = self.setup_token().await?;
            tracing::warn!(
                "no admin account yet: open {} and enter the one-time setup token {token}",
                self.cfg.url("/setup")
            );
        }
        jobs::spawn(self.clone());
        let service = self.router().into_make_service_with_connect_info::<SocketAddr>();
        Ok(axum::serve(listener, service).with_graceful_shutdown(shutdown()).await?)
    }

    pub async fn setup_token(&self) -> anyhow::Result<String> {
        let token = creds::token('s');
        let hash = creds::digest(&token);
        let sql = "INSERT INTO setup_tokens (hash, expires_at) VALUES (?1, ?2)";
        let stored = self.db.run(move |c| Ok(c.execute(sql, params![hash, db::now() + db::DAY])?));
        stored.await.map_err(|e| anyhow::anyhow!(e.detail))?;
        Ok(token)
    }

    pub async fn publish(self: &Arc<App>) -> anyhow::Result<Option<u64>> {
        let published = jobs::republish(self, true).await.map_err(|e| anyhow::anyhow!(e.detail))?;
        Ok(published.map(|p| p.version))
    }
}

async fn shutdown() {
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
    let term = async move {
        match term.as_mut() {
            Some(signal) => drop(signal.recv().await),
            None => std::future::pending().await,
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        () = term => {}
    }
}
