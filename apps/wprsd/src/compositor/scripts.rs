use super::lock;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};
use wprs_scene::ScriptOutputs;
use wprs_script::{Error, Frame, Host, Setup};

const SCRIPT_HOST: &str = "/proc/self/exe";
const LEAD: Duration = Duration::from_secs(1);

pub(super) struct Shared {
    host: Host,
    lead: (String, Instant),
    made: u64,
    last: Option<ScriptOutputs>,
    off: Option<Error>,
}

pub(super) struct Link {
    shared: Arc<Mutex<Shared>>,
    seen: u64,
    key: Key,
}

type Key = (usize, String);
pub(super) type Scripts = HashMap<Key, Weak<Mutex<Shared>>>;

pub(super) fn link(scripts: &mut Scripts, key: Key, setup: impl FnOnce() -> Setup) -> Link {
    scripts.retain(|_, w| w.strong_count() > 0);
    let shared = scripts.get(&key).and_then(Weak::upgrade).unwrap_or_else(|| {
        let host = Host::new(SCRIPT_HOST, &["--script-host"], &setup());
        let made = Arc::new(Mutex::new(Shared {
            host,
            lead: (String::new(), Instant::now()),
            made: 0,
            last: None,
            off: None,
        }));
        scripts.insert(key.clone(), Arc::downgrade(&made));
        made
    });
    Link { shared, seen: 0, key }
}

impl Link {
    pub(super) fn rekey(
        self,
        scripts: &mut Scripts,
        key: Key,
        setup: impl FnOnce() -> Setup,
    ) -> (Link, bool) {
        if self.key == key {
            return (self, true);
        }
        scripts.retain(|_, w| w.strong_count() > 0);
        if Arc::strong_count(&self.shared) == 1 && !scripts.contains_key(&key) {
            scripts.remove(&self.key);
            scripts.insert(key.clone(), Arc::downgrade(&self.shared));
            return (Link { key, ..self }, true);
        }
        drop(self);
        (link(scripts, key, setup), false)
    }

    pub(super) fn leads(&self, output: &str, inside: bool) -> bool {
        let s = lock(&self.shared);
        s.off.is_none() && (s.lead.0 == output || s.lead.1.elapsed() > LEAD || inside)
    }

    pub(super) fn poll<'a>(
        &mut self,
        output: &str,
        inside: bool,
        frame: impl FnOnce() -> Frame<'a>,
    ) -> Result<Option<ScriptOutputs>, Error> {
        let mut s = lock(&self.shared);
        if let Some(off) = &s.off {
            return Err(off.clone());
        }
        if s.lead.0 == output || s.lead.1.elapsed() > LEAD || inside {
            s.lead = (output.to_owned(), Instant::now());
            match s.host.update(&frame()) {
                Ok(Some(out)) => (s.made, s.last) = (s.made + 1, Some(out)),
                Ok(None) => {}
                Err(Error::Disabled) => {
                    s.off = Some(Error::Disabled);
                    return Err(Error::Disabled);
                }
                Err(e) => return Err(e),
            }
        }
        let fresh = s.made > self.seen;
        self.seen = s.made;
        Ok(s.last.clone().filter(|_| fresh))
    }

    pub(super) fn progress(&self) -> (u64, u64) {
        lock(&self.shared).host.progress()
    }

    #[cfg(test)]
    pub(super) fn shares(&self) -> (usize, usize) {
        (Arc::strong_count(&self.shared), Arc::as_ptr(&self.shared) as usize)
    }
}
