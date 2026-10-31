use super::*;

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "io.github.wallpaper_rs.Error")]
pub(super) enum Failure {
    #[zbus(error)]
    ZBus(zbus::Error),
    NotFound(String),
    Stale(String),
    Unconfirmed(String),
}

pub(super) struct Daemon(pub(super) Log);

#[zbus::interface(name = "io.github.wallpaper_rs.Daemon1")]
impl Daemon {
    #[zbus(property(emits_changed_signal = "const"))]
    fn model(&self) -> u32 {
        1
    }

    fn get_job(&self, job: u64) -> String {
        let state = if self.0.has(&format!("CancelJob {job}")) { "cancelled" } else { "done" };
        let output =
            json!({"reports": [{"source": null, "added": 2, "updated": 0, "moved": 0, "missing": 0}]});
        let job = json!({"id": job, "kind": "scan", "target": "", "state": state, "done": 2, "total": 2,
            "started_at": 1, "finished_at": 2, "error": null, "output": output});
        json!({"model": 1, "serial": 9, "job": job}).to_string()
    }

    async fn cancel_job(&self, job: u64, #[zbus(connection)] bus: &Connection) {
        self.0.push(format!("CancelJob {job}"));
        signal(bus, "JobFinished", &(job, "cancelled", "", "cancelled")).await;
    }

    fn set_option(&self, key: String, value: String) -> zbus::fdo::Result<()> {
        self.0.push(format!("SetOption {key} {value}"));
        Err(zbus::fdo::Error::InvalidArgs(format!("invalid value {value:?} for {key}")))
    }

    fn get_schema(&self) -> String {
        let immich = json!({"kind": "immich", "hosts": [], "options": [],
            "secret": {"name": "api_key", "required": true}});
        json!({"model": 1, "serial": 9, "options": [], "rules": [], "auto": [], "preset": [], "providers": [immich],
            "query": [], "thumb_sizes": [256, 512, 1024], "fits": ["fill"]})
        .to_string()
    }
}

pub(super) struct Library(pub(super) Log);

#[zbus::interface(name = "io.github.wallpaper_rs.Library1")]
impl Library {
    fn resolve(&self, refs: Vec<String>) -> Result<Vec<String>, Failure> {
        self.0.push(format!("Resolve {}", refs.join(" ")));
        let find = |r: &String| match r.ends_with(".png") {
            true => Some(UIDS[2].to_string()),
            false => UIDS.iter().find(|uid| uid.ends_with(&r.to_uppercase())).map(|uid| uid.to_string()),
        };
        let found =
            refs.iter().map(|r| find(r).ok_or_else(|| Failure::NotFound(format!("no item matches \"{r}\""))));
        found.collect()
    }

    async fn rescan(&self, source: String, #[zbus(connection)] bus: &Connection) -> u64 {
        self.0.push(format!("Rescan {source}"));
        let (bus, log, job) = (bus.clone(), self.0.clone(), if source == "slow" { 7 } else { 1 });
        tokio::spawn(async move {
            let mut done = 0_u64;
            while !log.has(&format!("CancelJob {job}")) {
                tokio::time::sleep(Duration::from_millis(50)).await;
                done = (done + 1).min(2);
                signal(&bus, "JobProgress", &(job, done, 2_u64)).await;
                if job == 1 && done == 2 {
                    return signal(&bus, "JobFinished", &(job, "done", "", "")).await;
                }
            }
        });
        job
    }

    fn search(&self, query: String, offset: u32, limit: u32) -> String {
        self.0.push(format!("Search {query} {offset} {limit}"));
        let item = |uid: &&str| {
            json!({"uid": uid, "kind": "image", "title": format!("Sea {}", &uid[20..]),
            "width": 64, "height": 32, "duration_ms": null, "color": null, "tags": ["sea"], "rating": 0,
            "favorite": false, "missing": false, "error": null, "revoked": null, "removable": true,
            "imported": false, "thumb": null})
        };
        let items: Vec<_> = UIDS.iter().skip(offset as usize).take(limit as usize).map(item).collect();
        let seeded = query.split(' ').map(|word| if word == "sort:random" { "sort:random:7" } else { word });
        let query = seeded.collect::<Vec<_>>().join(" ");
        json!({"model": 1, "serial": 9, "query": query, "total": UIDS.len(), "offset": offset, "items": items})
            .to_string()
    }
}
