use super::super::{Command, lock};
use super::view::ViewSlot;
use crate::pages::PageSpec;
use std::collections::BTreeMap;
use std::os::fd::OwnedFd;
use std::process::Child;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use wprs_web::{Allowed, FromWeb, Isolation, Link, Settings, ToWeb};

pub(super) const RETRIES: u32 = 3;
pub(super) const FIRST_FRAME: Duration = Duration::from_secs(10);
const BACKOFF: [u64; 3] = [1, 5, 30];

pub(super) type Inbox = Arc<Mutex<Vec<(FromWeb, Vec<OwnedFd>)>>>;

pub(in crate::compositor) struct PageSlot {
    pub(in crate::compositor) spec: PageSpec,
    pub(in crate::compositor) link: Option<Arc<Link>>,
    pub(in crate::compositor) child: Option<Child>,
    pub(in crate::compositor) inbox: Inbox,
    pub(in crate::compositor) views: BTreeMap<String, ViewSlot>,
    pub(in crate::compositor) next: u32,
    pub(in crate::compositor) error: Option<String>,
    pub(in crate::compositor) failures: u32,
    pub(in crate::compositor) due: Option<Instant>,
    pub(in crate::compositor) waiting: Option<Instant>,
    pub(in crate::compositor) settings: Settings,
    pub(in crate::compositor) allowed: Allowed,
    pub(in crate::compositor) files: Vec<(String, std::path::PathBuf)>,
    pub(in crate::compositor) folders: BTreeMap<String, wprs_web::Folder>,
    pub(in crate::compositor) audio: bool,
    pub(in crate::compositor) heard: Option<Instant>,
    pub(in crate::compositor) sandbox: &'static str,
    pub(in crate::compositor) shared: bool,
    pub(in crate::compositor) hidden: bool,
    pub(in crate::compositor) salt: [u8; 32],
    pub(in crate::compositor) scanned: Option<Instant>,
}

impl PageSlot {
    pub(super) fn new(spec: PageSpec, now: Instant) -> PageSlot {
        let mut salt = [0u8; 32];
        let _ = getrandom_fill(&mut salt);
        PageSlot {
            spec,
            link: None,
            child: None,
            inbox: Inbox::default(),
            views: BTreeMap::new(),
            next: 1,
            error: crate::pages::unavailable(),
            failures: 0,
            due: crate::pages::unavailable().is_none().then_some(now),
            waiting: None,
            settings: Settings::default(),
            allowed: Allowed::default(),
            files: Vec::new(),
            folders: BTreeMap::new(),
            audio: false,
            heard: None,
            sandbox: "",
            shared: true,
            hidden: false,
            salt,
            scanned: None,
        }
    }

    pub(in crate::compositor) fn send(&self, message: &ToWeb) {
        if let Some(link) = &self.link
            && let Err(e) = link.send(message, &[])
        {
            tracing::debug!("cannot reach the web helper: {e}");
        }
    }

    pub(super) fn spawn(&mut self, start: wprs_web::Start, tx: &mpsc::Sender<Command>, now: Instant) {
        self.stop();
        let Some(program) = crate::pages::helper() else { return };
        let networked = self.spec.privacy.network && !self.spec.pack.network.is_empty();
        let isolation = Isolation { network: !networked, devices: self.hidden };
        let zone = !self.spec.privacy.zone;
        let spawned = wprs_web::spawn(&program, isolation, |c| {
            if zone {
                c.env("TZ", "UTC");
            }
        });
        let (link, child) = match spawned {
            Ok(spawned) => spawned,
            Err(e) => return self.fail(format!("the web helper does not start: {e}"), now),
        };
        let link = Arc::new(link);
        if let Err(e) = link.send(&ToWeb::Start(Box::new(start)), &[]) {
            return self.fail(format!("the web helper does not answer: {e}"), now);
        }
        let isolated = isolation.network && wprs_web::isolated(child.id());
        self.sandbox = if isolated || !isolation.network { "on" } else { "reduced" };
        let (reader, inbox, tx) = (link.clone(), self.inbox.clone(), tx.clone());
        let spawned = std::thread::Builder::new().name("wprs-web-link".into()).spawn(move || {
            loop {
                let got = reader.recv::<FromWeb>(true);
                let (message, ended) = match got {
                    Ok(Some(message)) => (message, false),
                    Ok(None) | Err(_) => {
                        ((FromWeb::Failed("the web helper stopped".into()), Vec::new()), true)
                    }
                };
                let first = {
                    let mut inbox = lock(&inbox);
                    inbox.push(message);
                    inbox.len() == 1
                };
                if first {
                    let _ = tx.send(Command::Wake);
                }
                if ended {
                    return;
                }
            }
        });
        if let Err(e) = spawned {
            return self.fail(format!("cannot read the web helper: {e}"), now);
        }
        (self.link, self.child, self.waiting) = (Some(link), Some(child), Some(now + FIRST_FRAME));
        self.views.values_mut().for_each(ViewSlot::reset);
        (self.files, self.folders, self.settings, self.allowed) = Default::default();
    }

    pub(super) fn stop(&mut self) {
        self.send(&ToWeb::Quit);
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.link = None;
        lock(&self.inbox).clear();
    }

    pub(super) fn fail(&mut self, error: String, now: Instant) {
        self.stop();
        self.failures += 1;
        let retry = self.failures <= RETRIES;
        tracing::warn!(retry, "the web page failed: {error}");
        self.due = retry.then(|| now + Duration::from_secs(BACKOFF[(self.failures as usize - 1).min(2)]));
        self.error = Some(if retry { error } else { crate::words::web_failed() });
    }
}

impl Drop for PageSlot {
    fn drop(&mut self) {
        self.stop();
    }
}

fn getrandom_fill(salt: &mut [u8; 32]) -> std::io::Result<()> {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")?.read_exact(salt)
}
