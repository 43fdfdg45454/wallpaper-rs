use crate::cli::{Command, Wait};
use crate::input::path_arg;
use crate::{inputs, library, pkg, playlists, profiles, props, providers, remote, server, show};
use anyhow::{Context, Result, bail};
use futures_util::TryFutureExt;
use serde_json::{Value, json};
use std::cell::Cell;
use std::io::{self, IsTerminal};
use std::pin::pin;
use tokio::signal::unix::{SignalKind, signal};
use wprs_client::{Client, explain};
use wprs_core::Fit;
use zbus::export::ordered_stream::{OrderedStreamExt, join};

pub async fn run(command: Command) -> Result<()> {
    let client = &Client::connect().await?;
    let daemon = &client.daemon;
    match command {
        Command::Set { path, output, fit } => {
            daemon.set_wallpaper(&path_arg(&path)?, &output, fit.map_or("", Fit::as_str)).await?
        }
        Command::Fit { fit, output } => daemon.set_wallpaper("", &output, fit.as_str()).await?,
        Command::Background { color, output } => daemon.set_background(&color, &output).await?,
        Command::Config { key: Some(key), value: Some(value), .. } => daemon.set_option(&key, &value).await?,
        Command::Config { key, json, .. } => options(&daemon.get_state().await?, key.as_deref(), json)?,
        Command::Reload => daemon.reload().await?,
        Command::State { .. } => println!("{}", daemon.get_state().await?),
        Command::Pause => daemon.set_paused(true).await?,
        Command::Resume => daemon.set_paused(false).await?,
        Command::Toggle => daemon.set_paused(!daemon.paused().await?).await?,
        Command::Volume { percent } => {
            daemon.set_volume(f64::from(percent) / 100.0).await?;
            daemon.set_muted(false).await?
        }
        Command::Mute => daemon.set_muted(true).await?,
        Command::Unmute => daemon.set_muted(false).await?,
        Command::Stream { output, width, height, wait } => {
            let interrupted = wait.then(interrupted).transpose()?;
            let (node, serial) = match wait {
                true => daemon.request_stream(&output, width, height).await?,
                false => daemon.hold_stream(&output, width, height).await?,
            };
            println!("node {node} serial {serial}");
            if let Some(interrupted) = interrupted {
                interrupted.await;
                daemon.release_stream(&output).await?
            }
        }
        Command::Release { output } => daemon.release_stream(&output).await?,
        Command::Covered { output, covered } => daemon.set_output_covered(&output, covered).await?,
        Command::Screen { output, pos, size, scale, density, identity } => {
            let point =
                pos.split_once(',').and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)));
            let (x, y) = point.ok_or_else(|| crate::usage(format!("{pos} is not X,Y")))?;
            let (width, height) = crate::editing::dimensions(&size)?;
            daemon.set_output_info(&output, x, y, width, height, scale, density, &identity).await?
        }
        Command::Schema { json } => show::reply(daemon.get_schema().await?, json, show::schema)?,
        Command::Watch { json } => watch(client, json).await?,
        Command::Wait { serial, timeout, json } => {
            let serial = match serial {
                Some(serial) => serial,
                None => daemon.serial().await?,
            };
            let (serial, topics) = daemon.wait_changed(serial, timeout).await?;
            match json {
                true => println!("{}", json!({"serial": serial, "topics": topics})),
                false => println!("{}", format!("{serial} {}", topics.join(" ")).trim_end()),
            }
        }
        Command::Jobs { json } => show::reply(daemon.list_jobs().await?, json, show::jobs)?,
        Command::Job { id, json } => show::reply(daemon.get_job(id).await?, json, show::job)?,
        Command::Cancel { id } => daemon.cancel_job(id).await?,
        Command::Prop(command) => props::run(client, command).await?,
        Command::Lib(command) => library::run(client, command).await?,
        Command::Preset(command) => profiles::preset(client, command).await?,
        Command::Playlist(command) => playlists::run(client, command).await?,
        Command::Next(out) => client.playlists.next(&out.output).await?,
        Command::Prev(out) => client.playlists.previous(&out.output).await?,
        Command::Profile(command) => profiles::run(client, command).await?,
        Command::Provider(command) => providers::run(client, command).await?,
        Command::Remote(command) => remote::run(client, command).await?,
        Command::Pkg(command) => pkg::run(client, command).await?,
        Command::Input(command) => inputs::run(client, command).await?,
        Command::Server(command) => server::run(client, command).await?,
        Command::Pack(_) | Command::Repo(_) | Command::Edit(_) | Command::Scene(_) => {
            unreachable!("local commands never connect")
        }
    }
    Ok(())
}

pub async fn job<T>(
    client: &Client,
    start: impl Future<Output = zbus::Result<(T, u64)>>,
    wait: &Wait,
) -> Result<T> {
    if !wait.wait {
        let (value, id) = start.await.map_err(explain)?;
        println!("{id}");
        return Ok(value);
    }
    let started = Cell::new(None);
    let tty = io::stderr().is_terminal();
    let start = start.inspect_ok(|(_, id)| started.set(Some(*id)));
    let progress = |done, total| {
        if tty {
            eprint!("\r\x1b[Kjob {} {}", started.get().unwrap_or_default(), show::progress(done, total))
        }
    };
    let finished = tokio::select! {
        finished = client.run_job(start, progress) => Some(finished),
        _ = tokio::signal::ctrl_c() => None,
    };
    if tty {
        eprint!("\r\x1b[K");
    }
    let (value, job) = match (finished, started.get()) {
        (Some(finished), _) => finished?,
        (None, Some(id)) => bail!("stopped waiting; job {id} keeps running (wprs-ctl cancel {id} stops it)"),
        (None, None) => bail!("stopped waiting"),
    };
    println!("{}", if wait.json { serde_json::to_string(&job)? } else { show::summary(&job) });
    Ok(value)
}

async fn watch(client: &Client, json: bool) -> Result<()> {
    let daemon = &client.daemon;
    let line =
        |value: Value, text: String| if json { value.to_string() } else { show::clean(text.trim_end()) };
    let changed = daemon.receive_changed().await?.filter_map(|s| {
        let a = s.args().ok()?;
        let value = json!({"signal": "Changed", "topic": a.topic, "ids": a.ids, "serial": a.serial});
        Some(line(value, format!("{} {} {}", a.serial, a.topic, a.ids.join(" "))))
    });
    let progressed = daemon.receive_job_progress().await?.filter_map(|s| {
        let a = s.args().ok()?;
        let value = json!({"signal": "JobProgress", "job": a.job, "done": a.done, "total": a.total});
        Some(line(value, format!("job {} {}/{}", a.job, a.done, a.total)))
    });
    let finished = daemon.receive_job_finished().await?.filter_map(|s| {
        let a = s.args().ok()?;
        let value = json!({"signal": "JobFinished", "job": a.job, "state": a.state, "error_name": a.error_name,
            "message": a.message});
        Some(line(value, format!("job {} {} {}", a.job, a.state, a.message)))
    });
    let mut events = pin!(join(join(changed, progressed), finished));
    while let Some(line) = events.next().await {
        println!("{line}");
    }
    Ok(())
}

fn interrupted() -> io::Result<impl Future<Output = ()>> {
    let (mut interrupt, mut terminate) = (signal(SignalKind::interrupt())?, signal(SignalKind::terminate())?);
    Ok(async move {
        tokio::select! {
            _ = interrupt.recv() => {}
            _ = terminate.recv() => {}
        }
    })
}

fn options(state: &str, filter: Option<&str>, json: bool) -> Result<()> {
    let state: Value = serde_json::from_str(state)?;
    let options = state["options"].as_object().context("the daemon did not report its settings")?;
    if let Some(key) = filter.filter(|key| !options.contains_key(*key)) {
        bail!("unknown setting '{key}'; the settings are {}", show::list(options.keys()));
    }
    let shown = options.iter().filter(|(key, _)| filter.is_none_or(|f| key.as_str() == f));
    match json {
        true => println!("{}", Value::Object(shown.map(|(k, v)| (k.clone(), v.clone())).collect())),
        false => shown.for_each(|(key, value)| println!("{key} = {value}")),
    }
    Ok(())
}
