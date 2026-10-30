mod mock;

use mock::Bus;
use serde_json::json;
use std::time::Duration;
use wprs_client::{
    Client, Daemon1Proxy, Library1Proxy, Playlists1Proxy, Profiles1Proxy, Providers1Proxy, Repos1Proxy,
    error_name, explain, parse,
};
use wprs_core::dbus::{
    BUS_NAME, INTERFACE, LIBRARY, OBJECT_PATH, PLAYLISTS, PROFILES, PROVIDERS, REPOS, error,
};
use wprs_core::model::{JobState, TagsReply};
use zbus::proxy::Defaults;

async fn within<T>(work: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), work).await.expect("the job ends in time")
}

#[tokio::test]
async fn a_job_that_finishes_before_the_reply_is_not_missed() {
    let bus = Bus::start();
    let _daemon = bus.serve(Some(1)).await;
    let client = Client::new(bus.connect().await).await.unwrap();
    let mut seen = Vec::new();
    let rescan = async { Ok(((), client.library.rescan("fast").await?)) };
    let ((), job) = within(client.run_job(rescan, |done, total| seen.push((done, total)))).await.unwrap();
    assert_eq!((job.id, job.state, job.output), (1, JobState::Done, Some(json!({"reports": []}))));
    assert_eq!(seen, [(1, 4), (2, 4)]);
}

#[tokio::test]
async fn failed_jobs_and_calls_keep_their_error_names() {
    let bus = Bus::start();
    let _daemon = bus.serve(Some(1)).await;
    let client = Client::new(bus.connect().await).await.unwrap();
    let mut seen = Vec::new();
    let late = async { client.library.rescan("late").await.map(|job| ((), job)) };
    let failed = within(client.run_job(late, |done, total| seen.push((done, total)))).await.unwrap_err();
    assert_eq!(
        (error_name(&failed), failed.to_string()),
        (Some(error::STALE), "the signed index expired".into())
    );
    assert_eq!(seen, [(3, 4)]);
    let missing = async { client.library.rescan("gone").await.map(|job| ((), job)) };
    let missing = client.run_job(missing, |_, _| {}).await.unwrap_err();
    assert_eq!(
        (error_name(&missing), missing.to_string()),
        (Some(error::NOT_FOUND), "no source gone".into())
    );
    let silent = async { client.library.rescan("silent").await.map(|job| ((), job)) };
    let silent = within(client.run_job(silent, |_, _| {})).await.unwrap_err();
    assert_eq!((error_name(&silent), silent.to_string()), (None, "failed without giving a reason".into()));
    assert_eq!(error::reason(error::STALE, " "), "failed without giving a reason (Stale)");
}

#[tokio::test]
async fn a_daemon_restart_while_waiting_fails_the_job() {
    let bus = Bus::start();
    let daemon = bus.serve(Some(1)).await;
    let client = Client::new(bus.connect().await).await.unwrap();
    let (running, mut started) = tokio::sync::mpsc::unbounded_channel();
    let stuck = async { client.library.rescan("stuck").await.map(|job| ((), job)) };
    let job = client.run_job(stuck, |_, _| running.send(()).unwrap());
    let restart = async {
        started.recv().await;
        daemon.close().await.unwrap();
        bus.serve(Some(1)).await
    };
    let (failed, _restarted) = within(async { tokio::join!(job, restart) }).await;
    assert_eq!(failed.unwrap_err().to_string(), "wprsd restarted while job 3 was running");
}

#[tokio::test]
async fn the_data_model_is_checked_when_connecting() {
    let bus = Bus::start();
    let absent = Client::new(bus.connect().await).await.unwrap_err().to_string();
    assert_eq!(absent, format!("wprsd is not running ({BUS_NAME} is not on the session bus)"));
    let hint = "but this client uses 1; install matching versions of wallpaper-rs";
    for (model, expected) in [(Some(2), 2), (None, 0)] {
        let daemon = bus.serve(model).await;
        let error = Client::new(bus.connect().await).await.unwrap_err();
        assert_eq!(error.to_string(), format!("wprsd uses data model {expected} {hint}"));
        daemon.close().await.unwrap();
    }
    let daemon = bus.serve(Some(1)).await;
    let client = Client::new(bus.connect().await).await.unwrap();
    let older = client.repos.list().await.map_err(explain).unwrap_err();
    assert_eq!(
        older.to_string(),
        "wprsd is older than this client; install matching versions of wallpaper-rs"
    );
    assert_eq!(error_name(&older), Some("org.freedesktop.DBus.Error.UnknownInterface"));
    daemon.close().await.unwrap();
}

#[test]
fn replies_are_parsed_after_their_data_model_is_checked() {
    let tags: TagsReply =
        parse(r#"{"model": 1, "serial": 5, "tags": [{"name": "sea", "count": 2}]}"#).unwrap();
    assert_eq!((tags.serial, tags.tags[0].count), (5, 2));
    let newer = parse::<TagsReply>(r#"{"model": 2, "serial": 5, "tags": []}"#).unwrap_err().to_string();
    assert!(newer.starts_with("wprsd uses data model 2 but this client uses 1"), "{newer}");
    assert!(parse::<TagsReply>(r#"{"serial": 5}"#).unwrap_err().to_string().contains("data model 0"));
}

fn names<P: Defaults>() -> [Option<&'static str>; 3] {
    [
        P::INTERFACE.as_ref().map(|i| i.as_str()),
        P::DESTINATION.as_ref().map(|d| d.as_str()),
        P::PATH.as_ref().map(|p| p.as_str()),
    ]
}

#[test]
fn every_proxy_uses_the_core_names() {
    let proxies = [names::<Daemon1Proxy>(), names::<Library1Proxy>(), names::<Playlists1Proxy>()]
        .into_iter()
        .chain([names::<Profiles1Proxy>(), names::<Providers1Proxy>(), names::<Repos1Proxy>()]);
    let interfaces = [INTERFACE, LIBRARY, PLAYLISTS, PROFILES, PROVIDERS, REPOS];
    for (proxy, interface) in proxies.zip(interfaces) {
        assert_eq!(proxy, [Some(interface), Some(BUS_NAME), Some(OBJECT_PATH)]);
    }
}
