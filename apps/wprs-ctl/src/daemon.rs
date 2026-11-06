use crate::cli::{Command, Wait};
use crate::input::path_arg;
use crate::{inputs, library, pkg, playlists, profiles, props, providers, remote, serving, show};
use anyhow::{Context, Result, bail};
use futures_util::TryFutureExt;
use serde_json::{Value, json};
use std::cell::Cell;
use std::io::{self, IsTerminal};
use std::pin::pin;
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
        Command::Filter(args) => crate::filtering::run(client, args).await?,
        Command::Config { key: Some(key), value: Some(value), .. } => {
            daemon.set_option(&key, &value).await?;
            crate::gpus::notice(client, &key).await?;
            crate::levels::notice(client, &key).await?
        }
        Command::Config { key, json, .. } => options(&daemon.get_state().await?, key.as_deref(), json)?,
        Command::Gpu(args) => crate::gpus::run(client, args).await?,
        Command::Quality(args) => crate::levels::run(client, args).await?,
        Command::Reload => daemon.reload().await?,
        Command::Restart => crate::gpus::restart(client).await?,
        Command::RetryEffects => daemon.retry_effects().await?,
        Command::State { .. } => {
            let state = daemon.get_state().await?;
            println!("{state}");
            if std::io::IsTerminal::is_terminal(&std::io::stderr()) {
                crate::levels::notes(&state);
            }
        }
        Command::Calibrate { state } => daemon.show_calibration(state == "on").await?,
        Command::Pause => daemon.set_paused(true).await?,
        Command::Resume => daemon.set_paused(false).await?,
        Command::Toggle => daemon.set_paused(!daemon.paused().await?).await?,
        Command::Volume { percent } => {
            daemon.set_volume(f64::from(percent) / 100.0).await?;
            daemon.set_muted(false).await?
        }
        c @ (Command::Mute | Command::Unmute) => daemon.set_muted(matches!(c, Command::Mute)).await?,
        Command::Stream(crate::args::Streaming { output, width, height, wait }) => {
            let interrupted = wait.then(crate::args::interrupted).transpose()?;
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
        Command::Covered(crate::args::Cover { output, covered }) => {
            daemon.set_output_covered(&output, covered).await?
        }
        Command::Screen(placement) => placement.tell(daemon).await?,
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
        Command::Preset(command) => crate::presets::preset(client, command).await?,
        Command::Playlist(command) => playlists::run(client, command).await?,
        Command::Next(out) => inputs::asking(client, &out.output, client.playlists.next(&out.output)).await?,
        Command::Prev(out) => {
            inputs::asking(client, &out.output, client.playlists.previous(&out.output)).await?
        }
        Command::Profile(command) => profiles::run(client, command).await?,
        Command::Provider(command) => providers::run(client, command).await?,
        Command::Remote(command) => remote::run(client, command).await?,
        Command::Pkg(command) => pkg::run(client, command).await?,
        Command::Input(command) => inputs::run(client, command).await?,
        Command::Rgb(command) => crate::rgb::run(client, command).await?,
        Command::Media(command) => crate::media::run(client, command).await?,
        Command::Server(command) => serving::run(client, command).await?,
        Command::Pack(_)
        | Command::Repo(_)
        | Command::Edit(_)
        | Command::Scene(_)
        | Command::Models(_)
        | Command::Moderate(_) => {
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
    Ok(finished(client, start, wait).await?.0)
}

pub async fn finished<T>(
    client: &Client,
    start: impl Future<Output = zbus::Result<(T, u64)>>,
    wait: &Wait,
) -> Result<(T, Option<wprs_core::model::Job>)> {
    if !wait.wait {
        let (value, id) = start.await.map_err(explain)?;
        println!("{id}");
        return Ok((value, None));
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
    Ok((value, Some(job)))
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

fn options(state: &str, filter: Option<&str>, json: bool) -> Result<()> {
    let state: Value = serde_json::from_str(state)?;
    let options = state["options"].as_object().context("the daemon did not report its settings")?;
    if filter.is_some_and(|key| crate::levels::own(&state, key, json)) {
        return Ok(());
    }
    let unknown = |k: &str| format!("unknown setting '{k}'; the settings are {}", show::list(options.keys()));
    if let Some(key) = filter.filter(|key| !options.contains_key(*key)) {
        return Err(crate::usage(unknown(key)));
    }
    let shown = options.iter().filter(|(key, _)| filter.is_none_or(|f| key.as_str() == f));
    match json {
        true => println!("{}", Value::Object(shown.map(|(k, v)| (k.clone(), v.clone())).collect())),
        false => shown.for_each(|(key, value)| println!("{key} = {value}")),
    }
    if filter == Some(wprs_core::LayerNamespace::KEY) && !json {
        crate::gpus::in_use(&serde_json::from_value(state["layer_shell"].clone()).unwrap_or_default());
    }
    Ok(())
}
