use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn a_waited_job_shows_its_result() {
    let bus = Bus::start().await;
    assert_eq!(bus.ctl(&["lib", "scan"]), (0, "1\n".into(), String::new()));
    let (code, out, _) = bus.ctl(&["lib", "scan", "--wait"]);
    assert_eq!((code, out.as_str()), (0, "job 1 scan done: 2 added, 0 missing, 0 moved, 0 updated\n"));
    let (code, out, _) = bus.ctl(&["lib", "scan", "--wait", "--json"]);
    let job: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!((code, job["id"].as_u64(), job["state"].as_str()), (0, Some(1), Some("done")));
}

#[tokio::test(flavor = "multi_thread")]
async fn ctrl_c_stops_waiting_and_cancel_stops_the_job() {
    let bus = Bus::start().await;
    let waiting = bus.spawn(&["lib", "scan", "slow", "--wait"], "");
    let start = Instant::now();
    while !bus.log.has("Rescan slow") {
        assert!(start.elapsed() < Duration::from_secs(10), "the scan never started");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    Command::new("kill").args(["-INT", &waiting.id().to_string()]).status().unwrap();
    let (code, out, err) = texts(waiting.wait_with_output().unwrap());
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(err.contains("stopped waiting; job 7 keeps running (wprs-ctl cancel 7 stops it)"), "{err}");
    assert!(!bus.log.has("CancelJob"));
    assert_eq!(bus.ctl(&["cancel", "7"]).0, 0);
    assert!(bus.log.has("CancelJob 7"));
    let (code, out, _) = bus.ctl(&["job", "7"]);
    assert!(code == 0 && out.contains("state = cancelled\n"), "{out}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_search_page_prints_the_normalized_query() {
    let bus = Bus::start().await;
    let (code, out, err) =
        bus.ctl(&["lib", "search", "tag:sea sort:random", "--offset", "2", "--limit", "2"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        out,
        "query = tag:sea sort:random:7\nitems = 3-4 of 5\n\
         ITEM      KIND   SIZE   RATING  FLAGS  TITLE       TAGS\n\
         E1V0R3EF  image  64x32  0              Sea V0R3EF  sea\n\
         E1V0R4GH  image  64x32  0              Sea V0R4GH  sea\n"
    );
    assert!(bus.log.has("Search tag:sea sort:random 2 2"));
    let (_, out, _) = bus.ctl(&["lib", "search", "tag:sea sort:random:7", "--limit", "1", "--json"]);
    let page: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        (page["query"].as_str(), page["items"][0]["uid"].as_str()),
        (Some("tag:sea sort:random:7"), Some(UIDS[0]))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn ctrl_c_stops_the_stale_question_after_waiting() {
    let bus = Bus::start().await;
    let command = format!("{} pkg install org.example.dawn --wait", env!("CARGO_BIN_EXE_wprs-ctl"));
    let mut child = Command::new("script")
        .args(["-qfec", &command, "/dev/null"])
        .env("DBUS_SESSION_BUS_ADDRESS", &bus.address)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("script is installed");
    let (shown, mut stdout) = (Arc::new(Mutex::new(Vec::new())), child.stdout.take().unwrap());
    let reader = shown.clone();
    std::thread::spawn(move || {
        let mut chunk = [0; 512];
        while let Ok(n @ 1..) = std::io::Read::read(&mut stdout, &mut chunk) {
            reader.lock().unwrap().extend_from_slice(&chunk[..n]);
        }
    });
    let text = || String::from_utf8_lossy(&shown.lock().unwrap()).into_owned();
    let start = Instant::now();
    while !text().contains("Install anyway?") {
        assert!(start.elapsed() < Duration::from_secs(10), "no question: {}", text());
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    child.stdin.as_mut().unwrap().write_all(b"\x03").unwrap();
    let start = Instant::now();
    let status = loop {
        match child.try_wait().unwrap() {
            Some(status) => break status,
            None if start.elapsed() > Duration::from_secs(10) => {
                child.kill().ok();
                panic!("Ctrl-C did not stop the question: {}", text());
            }
            None => tokio::time::sleep(Duration::from_millis(20)).await,
        }
    };
    assert_eq!(status.code(), Some(1), "{}", text());
    assert!(text().contains("stopped without an answer"), "{}", text());
    assert!(!bus.log.has("Install org.example.dawn true"));
}
