use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::io::{IsTerminal, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use wprs_server::{App, Config};

#[derive(Parser)]
#[command(
    version = wprs_server::VERSION,
    about = "Self-hosted wallpaper-rs server: signed repository, catalog, uploads, moderation and device sync"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Run the server (the default)")]
    Serve,
    #[command(about = "Apply database migrations and exit")]
    Migrate,
    #[command(about = "Exit with 0 when the local server answers /healthz")]
    Healthcheck,
    #[command(about = "Print a new one-time token that creates an administrator at /setup")]
    SetupToken,
    #[command(subcommand, about = "Manage the signed package repository")]
    Repo(Repo),
    #[command(about = "Write a consistent snapshot of the database and every stored file")]
    Backup { dir: Option<PathBuf> },
    #[command(about = "Restore a snapshot made by backup while the server is stopped")]
    Restore {
        dir: PathBuf,
        #[arg(long, help = "Replace an existing database")]
        force: bool,
        #[arg(long, default_value_t = 1, help = "Signed index versions to publish after restoring")]
        advance: u32,
    },
    #[command(hide = true)]
    Validate {
        wpak: PathBuf,
        #[arg(long)]
        thumb: PathBuf,
        #[arg(long = "key")]
        keys: Vec<String>,
        #[arg(long)]
        require_signature: bool,
    },
}

#[derive(Subcommand)]
enum Repo {
    #[command(about = "Create the repository; prints the root secret key once, keep it offline")]
    Init {
        #[arg(long)]
        name: String,
        #[arg(long, help = "Accept only packages signed by an author the root lists")]
        require_author_signature: bool,
        #[arg(long, help = "Let accounts register author keys online, trusted per package on first install")]
        online_authors: bool,
    },
    #[command(about = "Import root files signed offline and, optionally, the online key")]
    Import {
        dir: PathBuf,
        #[arg(long)]
        online_key: Option<PathBuf>,
    },
    #[command(about = "Show the repository state and its share URL")]
    Status,
    #[command(about = "Sign and publish the index now")]
    Publish,
}

fn main() -> ExitCode {
    wprs_scene::serve_check();
    wprs_scene::check_out_of_process(true);
    let cli = Cli::parse();
    if let Some(Command::Validate { wpak, thumb, keys, require_signature }) = &cli.command {
        return match wprs_server::check(wpak, keys, *require_signature, thumb) {
            Ok(report) => print(serde_json::to_value(report).unwrap_or_default(), ExitCode::SUCCESS),
            Err(e) => print(serde_json::json!({"error": format!("{e:#}")}), ExitCode::from(2)),
        };
    }
    let filter = tracing_subscriber::EnvFilter::try_from_env("WPRS_LOG").unwrap_or_else(|_| "info".into());
    tracing_subscriber::fmt().with_env_filter(filter).with_ansi(std::io::stdout().is_terminal()).init();
    match run(cli.command.unwrap_or(Command::Serve)) {
        Ok(text) => print(text.into(), ExitCode::SUCCESS),
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn print(value: serde_json::Value, code: ExitCode) -> ExitCode {
    match value.as_str() {
        Some("") => {}
        Some(text) => println!("{text}"),
        None => println!("{value}"),
    }
    code
}

fn run(command: Command) -> Result<String> {
    let cfg = Config::from_env()?;
    let runtime = || tokio::runtime::Builder::new_multi_thread().enable_all().build();
    match command {
        Command::Serve => runtime()?.block_on(async {
            let listener = tokio::net::TcpListener::bind(cfg.listen)
                .await
                .with_context(|| format!("cannot listen on {}", cfg.listen))?;
            tracing::info!("listening on {} for {}", cfg.listen, cfg.public_url);
            App::open(cfg)?.serve(listener).await.map(|()| String::new())
        }),
        Command::Migrate => App::open(cfg).map(|_| "The database is up to date".into()),
        Command::Healthcheck => healthcheck(cfg.listen.port()).map(|()| String::new()),
        Command::SetupToken => runtime()?.block_on(App::open(cfg)?.setup_token()),
        Command::Repo(Repo::Init { name, require_author_signature, online_authors }) => {
            App::open(cfg.clone())?;
            let root = wprs_server::init(&cfg, &name, require_author_signature, online_authors)?;
            let status = wprs_server::status(&cfg)?;
            Ok(format!(
                "Root key fingerprint: {}\n\
                 Store this root secret key offline now, the server does not keep it:\n\n{}\n{status}",
                root.fingerprint, root.secret
            ))
        }
        Command::Repo(Repo::Import { dir, online_key }) => {
            let status = wprs_server::import(&cfg, &dir, online_key.as_deref())?;
            Ok(format!("Imported root {} of repository {}", status.version, status.repo_id))
        }
        Command::Repo(Repo::Status) => Ok(wprs_server::status(&cfg)?.to_string()),
        Command::Repo(Repo::Publish) => {
            let version = runtime()?
                .block_on(App::open(cfg)?.publish())?
                .context("no repository yet: run wprs-server repo init")?;
            Ok(format!("Published signed index version {version}"))
        }
        Command::Backup { dir } => Ok(wprs_server::backup(&cfg, dir)?.display().to_string()),
        Command::Restore { dir, force, advance } => {
            wprs_server::restore(&cfg, &dir, force)?;
            let (app, runtime) = (App::open(cfg)?, runtime()?);
            for _ in 0..advance {
                runtime.block_on(app.publish())?;
            }
            Ok(format!("Restored {}", dir.display()))
        }
        Command::Validate { .. } => bail!("validate runs on its own"),
    }
}

fn healthcheck(port: u16) -> Result<()> {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(3))?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.write_all(b"GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n")?;
    let mut reply = String::new();
    stream.take(256).read_to_string(&mut reply)?;
    if reply.starts_with("HTTP/1.0 200") || reply.starts_with("HTTP/1.1 200") {
        Ok(())
    } else {
        bail!("unhealthy: {}", reply.lines().next().unwrap_or_default())
    }
}
